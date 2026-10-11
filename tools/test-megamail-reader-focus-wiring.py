#!/usr/bin/env python3
# SPDX-License-Identifier: AGPL-3.0-or-later
"""Source-only lifecycle guards. NOT Rust/GPUI/focus/clipboard acceptance."""
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parent.parent
SOURCE = (ROOT / "apps/megamail/src/main.rs").read_text()


def function(name):
    start = SOURCE.index("    fn " + name + "(")
    end = SOURCE.find("\n    fn ", start + 1)
    return SOURCE[start:end if end != -1 else len(SOURCE)]


class NativeFocusWiring(unittest.TestCase):
    def test_immediate_start_and_refusal_use_synchronizing_retirement(self):
        block = function("open_original_formatting")
        self.assertIn("self.retire_reader_session(cx);", block)
        self.assertIn("this.retire_reader_session(cx);", block)
        self.assertNotIn("reader_session.retire()", block)

    def test_retirement_refreshes_actual_consumer_after_generation_changes(self):
        block = function("retire_reader_session")
        self.assertLess(block.index("reader_session.retire()"), block.index("self.refresh_reader_text(&snapshot, cx)"))
        self.assertIn("self.live.read(cx).snapshot()", block)

    def test_poll_and_hidden_view_retirement_refresh_before_next_render(self):
        before = SOURCE.index("let reader_generation_before_poll = this.reader_session.generation;")
        poll = SOURCE.index("for url in this.reader_session.poll()", before)
        condition = SOURCE.index("if reader_generation_before_poll != this.reader_session.generation", poll)
        refresh = SOURCE.index("this.refresh_reader_text(&snapshot, cx);", condition)
        notify = SOURCE.index("if previous_recovery_status", condition)
        self.assertLess(refresh, notify)
        self.assertIn("this.retire_reader_session(cx);", SOURCE[before - 350:before])

    def test_helper_exhaustion_does_not_disable_native_identity(self):
        block = function("reader_text_identity")
        self.assertNotIn("self.reader_session.exhausted", block)
        for field in ("key: key.clone()", "version: snapshot.selected_body_version", "generation: self.reader_session.generation"):
            self.assertIn(field, block)

    def test_live_body_retirement_has_post_projection_refresh(self):
        block = function("on_live_change")
        self.assertLess(block.index("self.reader_session.retire()"), block.rindex("self.refresh_reader_text(&snapshot, cx)"))
        self.assertEqual(SOURCE.count("reader_session.retire();"), 2,
                         "additional direct retire sites need an explicit synchronization review")

    def test_managed_renderer_and_action_use_actual_same_state(self):
        self.assertIn("TextView::new(&target.state)", SOURCE)
        start = SOURCE.index(".on_action(cx.listener(|this, _: &FocusReader")
        block = SOURCE[start:start + 1800]
        self.assertIn("target.state.read(cx).focus_handle().clone()", block)
        self.assertIn("target.matches(&identity)", block)


if __name__ == "__main__":
    unittest.main(verbosity=2)
