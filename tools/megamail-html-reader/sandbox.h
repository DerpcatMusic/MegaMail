/* SPDX-License-Identifier: AGPL-3.0-or-later */
#pragma once
#include <glib.h>
/* Re-execs fixed bubblewrap arguments; never interprets mail as paths/commands. */
gboolean mm_enter_sandbox(const char *self, GError **error);
gboolean mm_enter_snapshot(const char *self, GError **error);
gboolean mm_verify_sandbox(GError **error);
