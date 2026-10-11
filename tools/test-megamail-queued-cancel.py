#!/usr/bin/env python3
"""Exercise the production schedule-cancellation SQL without compiling Rust."""
from pathlib import Path
import re
import sqlite3
import unittest

SOURCE = (Path(__file__).resolve().parents[1] / 'src/cache.rs').read_text()
SQL = re.search(r'pub fn cancel_queued_outbox\b.*?tx\.execute\(\s*"([^"\n]+)"', SOURCE, re.S).group(1)

class Cancellation(unittest.TestCase):
    def setUp(self):
        self.db = sqlite3.connect(':memory:')
        for table in ('outbox', 'outbox_delivery'):
            schema = re.search(r'CREATE TABLE IF NOT EXISTS ' + table + r' \(.*?\n\);', SOURCE, re.S).group(0)
            self.db.execute(schema)
        trigger = re.search(r'CREATE TRIGGER IF NOT EXISTS outbox_delivery_delete.*?\nEND;', SOURCE, re.S).group(0)
        self.db.executescript(trigger)
        self.db.execute("INSERT INTO outbox(account_id,from_addr,rcpts,recipients,subject,raw,queued_at,send_at) VALUES(7,'owner@example.test','recipient@example.test','recipient','Fixture',?,1000,2000)", (b'private raw',))
        self.id = self.db.execute('SELECT id FROM outbox').fetchone()[0]
        self.db.execute("INSERT INTO outbox_delivery VALUES(7,?,'ready')", (self.id,))
        self.db.commit()

    def tearDown(self):
        self.db.close()

    def cancel(self, account=7, at=2000, now=1000):
        with self.db:
            return self.db.execute(SQL, (account, self.id, at, now)).rowcount

    def test_exact_ready_future_intent_and_side_state_are_removed_once(self):
        self.assertEqual(self.cancel(), 1)
        self.assertEqual(self.cancel(), 0)
        self.assertEqual(self.db.execute('SELECT COUNT(*) FROM outbox_delivery').fetchone()[0], 0)

    def test_unscheduled_ready_copy_can_be_canceled_but_not_as_a_schedule(self):
        self.db.execute('UPDATE outbox SET send_at=NULL')
        self.assertEqual(self.cancel(), 0)
        self.assertEqual(self.cancel(at=None), 1)

    def test_a_schedule_cannot_be_removed_as_an_unscheduled_copy(self):
        self.assertEqual(self.cancel(at=None), 0)
        self.assertEqual(self.db.execute('SELECT raw FROM outbox').fetchone()[0], b'private raw')

    def test_wrong_account_changed_time_and_due_intent_preserve_bytes(self):
        for args in ({'account': 8}, {'at': 2001}, {'now': 2000}, {'now': 2001}):
            self.assertEqual(self.cancel(**args), 0)
            self.assertEqual(self.db.execute('SELECT raw FROM outbox').fetchone()[0], b'private raw')

    def test_claimed_accepted_refused_uncertain_and_missing_phase_preserve_bytes(self):
        for phase in ('in_flight', 'accepted', 'refused', 'uncertain'):
            self.db.execute('UPDATE outbox_delivery SET state=?', (phase,))
            self.assertEqual(self.cancel(), 0)
        self.db.execute('DELETE FROM outbox_delivery')
        self.assertEqual(self.cancel(), 0)
        self.assertEqual(self.db.execute('SELECT raw FROM outbox').fetchone()[0], b'private raw')

if __name__ == '__main__':
    unittest.main()
