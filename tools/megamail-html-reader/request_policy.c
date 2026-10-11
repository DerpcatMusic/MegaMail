/* SPDX-License-Identifier: AGPL-3.0-or-later */
#include "policy.h"
#include <string.h>

gboolean mm_safe_link(const char *uri) {
    if (!uri || strlen(uri) > 4096) return FALSE;
    for (const char *p = uri; *p; ++p)
        if ((guchar)*p <= 32 || *p == '\\' || (guchar)*p == 127) return FALSE;
    GUri *u = g_uri_parse(uri, G_URI_FLAGS_NONE, NULL);
    if (!u) return FALSE;
    const char *scheme = g_uri_get_scheme(u), *host = g_uri_get_host(u);
    gboolean ok = scheme && (!g_ascii_strcasecmp(scheme, "https") || !g_ascii_strcasecmp(scheme, "http")) &&
                  host && *host && !g_uri_get_userinfo(u);
    g_uri_unref(u);
    return ok;
}
/* Exact capability membership, not prefix/path normalization. Production uses
 * this in WebPage::send-request (including redirects) and the scheme callback. */
gboolean mm_request_allowed(const char *uri, const char *token, guint count) {
    if (!uri || !token || count > MM_RESOURCES_MAX) return FALSE;
    if (!strcmp(uri, "about:blank")) return TRUE; /* trusted bootstrap only */
    char *expected = g_strdup_printf("megamail-resource://%s/document", token);
    gboolean ok = !strcmp(uri, expected);
    g_free(expected);
    for (guint i = 0; !ok && i < count; ++i) {
        expected = g_strdup_printf("megamail-resource://%s/part/%u", token, i);
        ok = !strcmp(uri, expected); g_free(expected);
    }
    return ok;
}
