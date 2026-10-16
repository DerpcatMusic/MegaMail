/* SPDX-License-Identifier: AGPL-3.0-or-later */
#include <webkit2/webkit-web-extension.h>
#include <gmodule.h>
#include <webkitdom/webkitdom.h>
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
static void document_loaded(WebKitWebPage *page, gpointer unused) {
    (void)unused;
    char *expected = g_strdup_printf("megamail-resource://%s/document", token);
    gboolean own = !g_strcmp0(webkit_web_page_get_uri(page), expected); g_free(expected);
    if (!own) return;
    WebKitDOMDocument *doc = webkit_web_page_get_dom_document(page);
    WebKitDOMElement *root = doc ? webkit_dom_document_get_document_element(doc) : NULL;
    if (!root) return;
    double height = webkit_dom_element_get_scroll_height(root);
    WebKitUserMessage *message = webkit_user_message_new("megamail-layout-v1", g_variant_new("(sd)", token, height));
    webkit_web_page_send_message_to_view(page, message, NULL, NULL, NULL);
}
static gboolean link_map(WebKitWebPage *page, WebKitUserMessage *message, gpointer unused) {
    (void)unused;
    GVariant *params = webkit_user_message_get_parameters(message);
    if (strcmp(webkit_user_message_get_name(message), "megamail-links-v1") || !params ||
        !g_variant_is_of_type(params, G_VARIANT_TYPE("(sii)"))) return FALSE;
    const char *input=NULL; int viewport_width=0, viewport_height=0;
    g_variant_get(params, "(&sii)", &input, &viewport_width, &viewport_height);
    if (g_strcmp0(input, token) || viewport_width<320 || viewport_width>1280 || viewport_height<1 || viewport_height>8192) return FALSE;
    WebKitDOMDocument *doc = webkit_web_page_get_dom_document(page);
    WebKitDOMHTMLCollection *anchors = doc ? webkit_dom_document_get_elements_by_tag_name_as_html_collection(doc, "a") : NULL;
    JsonBuilder *out = json_builder_new(); json_builder_begin_array(out);
    WebKitDOMElement *root_element=doc ? webkit_dom_document_get_document_element(doc) : NULL;
    double document_height=root_element ? webkit_dom_element_get_scroll_height(root_element) : 0;
    guint emitted = 0;
    for (gulong i=0; anchors && i<webkit_dom_html_collection_get_length(anchors) && i<4096 && emitted<256; ++i) {
        WebKitDOMElement *anchor = WEBKIT_DOM_ELEMENT(webkit_dom_html_collection_item(anchors, i));
        char *url = webkit_dom_element_get_attribute(anchor, "href");
        if (!url || !mm_safe_link(url) || strlen(url)>8192) { g_free(url); continue; }
        WebKitDOMClientRect *rect = webkit_dom_element_get_bounding_client_rect(anchor);
        double x=webkit_dom_client_rect_get_left(rect), y=webkit_dom_client_rect_get_top(rect),
            w=webkit_dom_client_rect_get_width(rect), h=webkit_dom_client_rect_get_height(rect);
        if (!(x>=0 && y>=0 && w>0 && h>0 && x+w<=viewport_width && y+h<=viewport_height)) { g_free(url); g_object_unref(rect); continue; }
        char *label = webkit_dom_node_get_text_content(WEBKIT_DOM_NODE(anchor));
        if (!label || strlen(label)>512) { g_free(label); label=g_strdup("Open message link"); }
        json_builder_begin_object(out);
        json_builder_set_member_name(out, "url"); json_builder_add_string_value(out, url);
        json_builder_set_member_name(out, "label"); json_builder_add_string_value(out, label);
        json_builder_set_member_name(out, "x"); json_builder_add_double_value(out, x);
        json_builder_set_member_name(out, "y"); json_builder_add_double_value(out, y);
        json_builder_set_member_name(out, "w"); json_builder_add_double_value(out, w);
        json_builder_set_member_name(out, "h"); json_builder_add_double_value(out, h);
        json_builder_end_object(out); ++emitted; g_free(url); g_free(label); g_object_unref(rect);
    }
    /* document-loaded can precede the first layout in an offscreen view.
     * Read geometry after forcing layout, then report the complete height. */
    if (root_element) {
        WebKitDOMClientRect *rect = webkit_dom_element_get_bounding_client_rect(root_element);
        document_height = MAX(webkit_dom_element_get_scroll_height(root_element), webkit_dom_client_rect_get_height(rect));
        g_object_unref(rect);
    }
    json_builder_end_array(out); JsonNode *root=json_builder_get_root(out);
    JsonGenerator *generator=json_generator_new(); json_generator_set_root(generator, root);
    char *json=json_generator_to_data(generator, NULL);
    WebKitUserMessage *reply=webkit_user_message_new("megamail-links-v1", g_variant_new("(ssd)", token, json, document_height));
    webkit_user_message_send_reply(message, reply);
    g_free(json); g_object_unref(generator); json_node_free(root); g_object_unref(out);
    if (anchors) g_object_unref(anchors);
    return TRUE;
}
static void page_created(WebKitWebExtension *extension, WebKitWebPage *page, gpointer unused) {
    (void)extension; (void)unused;
    g_signal_connect(page, "user-message-received", G_CALLBACK(link_map), NULL);
    g_signal_connect(page, "document-loaded", G_CALLBACK(document_loaded), NULL);
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
