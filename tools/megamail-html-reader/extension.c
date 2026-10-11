/* SPDX-License-Identifier: AGPL-3.0-or-later */
#include <webkit2/webkit-web-extension.h>
#include <gmodule.h>
#include <string.h>
#include "policy.h"
static char *token;
static guint count;
static gboolean cancel_request(WebKitWebPage *page, WebKitURIRequest *request,
                               WebKitURIResponse *redirect, gpointer unused) {
    (void)page; (void)redirect; (void)unused;
    /* TRUE cancels before dispatch; policy is applied again to every redirect. */
    return !mm_request_allowed(webkit_uri_request_get_uri(request), token, count);
}
static void page_created(WebKitWebExtension *extension, WebKitWebPage *page, gpointer unused) {
    (void)extension; (void)unused;
    g_signal_connect(page, "send-request", G_CALLBACK(cancel_request), NULL);
    /* Acknowledge only AFTER the cancellation hook is installed on this page. */
    WebKitUserMessage *message = webkit_user_message_new("megamail-policy-v1", g_variant_new_string(token));
    webkit_web_page_send_message_to_view(page, message, NULL, NULL, NULL);
}
G_MODULE_EXPORT void webkit_web_extension_initialize_with_user_data(
    WebKitWebExtension *extension, const GVariant *data) {
    const char *input = NULL;
    if (!data || !g_variant_is_of_type((GVariant *)data, G_VARIANT_TYPE("(su)"))) return;
    g_variant_get((GVariant *)data, "(&su)", &input, &count);
    if (!input || strlen(input) != 32 || count > MM_RESOURCES_MAX) return;
    for (const char *p = input; *p; ++p) if (!g_ascii_isxdigit(*p)) return;
    token = g_strdup(input);
    g_signal_connect(extension, "page-created", G_CALLBACK(page_created), NULL);
}
