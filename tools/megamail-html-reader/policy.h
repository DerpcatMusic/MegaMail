/* SPDX-License-Identifier: AGPL-3.0-or-later */
#pragma once
#include <glib.h>
#include <json-glib/json-glib.h>
#define MM_SOURCE_MAX (2u * 1024u * 1024u)
#define MM_WIRE_MAX (20u * 1024u * 1024u)
#define MM_RESOURCE_MAX (2u * 1024u * 1024u)
#define MM_RESOURCES_MAX 32u
#define MM_PIXELS_MAX (8u * 1024u * 1024u)
#define MM_TOTAL_PIXELS_MAX (16u * 1024u * 1024u)
#define MM_NORMALIZED_MAX (12u * 1024u * 1024u)
#ifdef MM_TEST_NO_CSP
/* Synthetic-only mutation build. Never accepted by the native host. */
#define MM_CSP ""
#define MM_READY_POLICY "test-only:no-csp"
#else
#define MM_READY_POLICY "v1:extension+sandbox+ephemeral+no-network"
#define MM_CSP "default-src 'none'; script-src 'none'; worker-src 'none'; connect-src 'none'; object-src 'none'; frame-src 'none'; child-src 'none'; form-action 'none'; base-uri 'none'; style-src 'unsafe-inline'; img-src megamail-resource:; font-src 'none'; media-src 'none'"
#endif
typedef struct { char *cid, *uri; GBytes *png; } MmResource;
typedef struct {
    char *html, *plain, *title, *token, *document_uri;
    GPtrArray *resources;
} MmDocument;
MmDocument *mm_document_parse(const char *wire, gsize length, const char *token, GError **error);
void mm_document_free(MmDocument *doc);
gboolean mm_safe_link(const char *uri);
gboolean mm_request_allowed(const char *uri, const char *token, guint resource_count);
char *mm_read_input(GError **error);
void mm_emit(const char *event, const char *token, const char *field, const char *value);
