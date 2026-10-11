/* SPDX-License-Identifier: AGPL-3.0-or-later */
#include "policy.h"
#include <stdio.h>
#include <string.h>
/* Called only by --self-test; exercises the exact functions used by production,
 * not a Python facsimile. Parent executes this after its one batch compilation. */
int mm_policy_self_test(void) {
    const char *token = "0123456789abcdef0123456789abcdef";
    guint checks = 0;
#define CHECK(x) do { ++checks; if (!(x)) { fprintf(stderr, "policy assertion %u failed\n", checks); return 1; } } while (0)
    CHECK(mm_request_allowed("about:blank", token, 1));
    CHECK(mm_request_allowed("megamail-resource://0123456789abcdef0123456789abcdef/document", token, 1));
    CHECK(mm_request_allowed("megamail-resource://0123456789abcdef0123456789abcdef/part/0", token, 1));
    const char *denied[] = {
        "http://127.0.0.1:48191/", "https://example.invalid/", "file:///etc/passwd",
        "data:image/png;base64,AAAA", "javascript:alert(1)", "ftp://example.invalid/",
        "megamail-resource://aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa/part/0",
        "megamail-resource://0123456789abcdef0123456789abcdef/part/1",
        "megamail-resource://0123456789abcdef0123456789abcdef/part/00",
        "megamail-resource://0123456789abcdef0123456789abcdef/part/%30",
        "megamail-resource://0123456789abcdef0123456789abcdef/part/../document",
        "megamail-resource://0123456789abcdef0123456789abcdef/document?x=1",
        "megamail-resource://0123456789abcdef0123456789abcdef/document#x",
    };
    for (guint i = 0; i < G_N_ELEMENTS(denied); ++i) CHECK(!mm_request_allowed(denied[i], token, 1));
    CHECK(!mm_request_allowed("about:blank", NULL, 1));
    CHECK(!mm_request_allowed("about:blank", token, MM_RESOURCES_MAX+1));
    CHECK(mm_safe_link("https://example.invalid/a?x=1"));
    const char *bad_links[] = {"file:///etc/passwd", "javascript:alert(1)", "data:text/html,hi", "mailto:a@example.invalid",
        "https://user:password@example.invalid/", "https://example.invalid/\n", "/relative", "https://example.invalid\\evil"};
    for (guint i = 0; i < G_N_ELEMENTS(bad_links); ++i) CHECK(!mm_safe_link(bad_links[i]));
    const char *wire = "{\"version\":1,\"html\":\"<style>@media(max-width:400px){td{display:block}}</style><table><tr><td onclick='bad()'>Text</td></tr></table><script>bad()</script>\",\"plainText\":\"Text\",\"title\":\"Fixture\"}";
    GError *error = NULL;
    MmDocument *doc = mm_document_parse(wire, strlen(wire), token, &error);
    CHECK(doc != NULL); CHECK(strstr(doc->html, "@media") != NULL);
    CHECK(strstr(doc->html, "<table") != NULL); CHECK(strstr(doc->html, "onclick") == NULL);
    CHECK(strstr(doc->html, "<script") == NULL); mm_document_free(doc);
    const char *bad_requests[] = {
        "{\"version\":2,\"html\":\"x\",\"plainText\":\"x\",\"title\":\"x\"}",
        "{\"version\":1,\"html\":\"x\\u0000y\",\"plainText\":\"x\",\"title\":\"x\"}",
        "{\"version\":1,\"html\":\"x\",\"plainText\":\"x\",\"title\":\"x\",\"profilePath\":\"/home/forbidden\"}",
    };
    for (guint i = 0; i < G_N_ELEMENTS(bad_requests); ++i) {
        CHECK(mm_document_parse(bad_requests[i], strlen(bad_requests[i]), token, &error) == NULL);
        g_clear_error(&error);
    }
    printf("{\"event\":\"policy-self-test\",\"assertions\":%u,\"engine_test\":false}\n", checks);
    return 0;
#undef CHECK
}
