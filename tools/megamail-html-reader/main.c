/* SPDX-License-Identifier: AGPL-3.0-or-later */
#include <webkit2/webkit2.h>
#include <gtk/gtk.h>
#include <sys/resource.h>
#include <unistd.h>
#include <string.h>
#include "policy.h"
#include "sandbox.h"

typedef struct {
    MmDocument *doc;
    WebKitWebView *view;
    GtkWidget *window, *stack, *plain_button, *notice;
    gboolean acknowledged, loading, ready, failed;
    guint deadline, links;
} Reader;
static void failed(Reader *r, const char *code) {
    if (r->failed) return;
    r->failed = TRUE;
    webkit_web_view_stop_loading(r->view);
    gtk_stack_set_visible_child_name(GTK_STACK(r->stack), "plain");
    gtk_label_set_text(GTK_LABEL(r->notice), "Original formatting unavailable. Showing plain text.");
    mm_emit("error", r->doc->token, "code", code);
}
static gboolean deadline(gpointer data) {
    Reader *r = data; r->deadline = 0;
    if (!r->ready) failed(r, "policy-or-load-timeout");
    return G_SOURCE_REMOVE;
}
static gboolean heartbeat(gpointer data) {
    Reader *r = data;
    if (r->ready && !r->failed) {
        if (!webkit_web_view_get_is_web_process_responsive(r->view)) failed(r, "web-process-unresponsive");
        else mm_emit("heartbeat", r->doc->token, NULL, NULL);
    }
    return G_SOURCE_CONTINUE;
}
static gboolean load_document(gpointer data) {
    Reader *r = data;
    if (!r->failed && r->acknowledged && !r->loading) {
        r->loading = TRUE;
        webkit_web_view_load_uri(r->view, r->doc->document_uri);
    }
    return G_SOURCE_REMOVE;
}
static gboolean policy_message(WebKitWebView *view, WebKitUserMessage *message, gpointer data) {
    (void)view; Reader *r = data;
    GVariant *p = webkit_user_message_get_parameters(message);
    if (strcmp(webkit_user_message_get_name(message), "megamail-policy-v1") || !p ||
        !g_variant_is_of_type(p, G_VARIANT_TYPE_STRING) || strcmp(g_variant_get_string(p, NULL), r->doc->token)) return FALSE;
    if (!r->acknowledged) {
        r->acknowledged = TRUE;
        g_idle_add(load_document, r);
    }
    return TRUE;
}
static void scheme_request(WebKitURISchemeRequest *request, gpointer data) {
    Reader *r = data;
    const char *uri = webkit_uri_scheme_request_get_uri(request);
    GBytes *bytes = NULL;
    const char *mime = "image/png";
    if (r->acknowledged && !r->failed && mm_request_allowed(uri, r->doc->token, r->doc->resources->len)) {
        if (!strcmp(uri, r->doc->document_uri)) {
            mime = "text/html; charset=utf-8";
            bytes = g_bytes_new_static(r->doc->html, strlen(r->doc->html));
        } else for (guint i = 0; i < r->doc->resources->len; ++i) {
            MmResource *resource = g_ptr_array_index(r->doc->resources, i);
            if (!strcmp(uri, resource->uri)) bytes = g_bytes_ref(resource->png);
        }
    }
    if (!bytes) {
        GError *error = g_error_new_literal(G_IO_ERROR, G_IO_ERROR_PERMISSION_DENIED, "resource-denied");
        webkit_uri_scheme_request_finish_error(request, error); g_error_free(error); return;
    }
    GInputStream *stream = g_memory_input_stream_new_from_bytes(bytes);
    WebKitURISchemeResponse *response = webkit_uri_scheme_response_new(stream, g_bytes_get_size(bytes));
    webkit_uri_scheme_response_set_content_type(response, mime);
    SoupMessageHeaders *headers = soup_message_headers_new(SOUP_MESSAGE_HEADERS_RESPONSE);
    soup_message_headers_append(headers, "Content-Security-Policy", MM_CSP);
    soup_message_headers_append(headers, "Cache-Control", "no-store");
    soup_message_headers_append(headers, "X-Content-Type-Options", "nosniff");
    soup_message_headers_append(headers, "Referrer-Policy", "no-referrer");
    /* Ownership of headers transfers to the response. */
    webkit_uri_scheme_response_set_http_headers(response, headers);
    webkit_uri_scheme_request_finish_with_response(request, response);
    g_object_unref(response); g_object_unref(stream); g_bytes_unref(bytes);
}
static gboolean decide_policy(WebKitWebView *view, WebKitPolicyDecision *decision,
                              WebKitPolicyDecisionType type, gpointer data) {
    (void)view; Reader *r = data;
    if (type == WEBKIT_POLICY_DECISION_TYPE_NAVIGATION_ACTION || type == WEBKIT_POLICY_DECISION_TYPE_NEW_WINDOW_ACTION) {
        WebKitNavigationAction *action = webkit_navigation_policy_decision_get_navigation_action(WEBKIT_NAVIGATION_POLICY_DECISION(decision));
        const char *uri = webkit_uri_request_get_uri(webkit_navigation_action_get_request(action));
        if (type == WEBKIT_POLICY_DECISION_TYPE_NAVIGATION_ACTION && !r->ready && !r->failed &&
            ((!r->loading && !strcmp(uri, "about:blank")) ||
             (r->loading && r->acknowledged && !strcmp(uri, r->doc->document_uri)))) {
            webkit_policy_decision_use(decision); return TRUE;
        }
        /* Never automatically navigate/open. Only engine-verified link clicks
         * in the loaded immutable document can become native handoff candidates. */
        if (r->ready && !r->failed && r->links < 256 &&
            webkit_navigation_action_get_navigation_type(action) == WEBKIT_NAVIGATION_TYPE_LINK_CLICKED &&
            webkit_navigation_action_is_user_gesture(action) && mm_safe_link(uri)) {
            ++r->links;
            mm_emit("user-click-link", r->doc->token, "url", uri);
        }
    } else if (type == WEBKIT_POLICY_DECISION_TYPE_RESPONSE) {
        WebKitResponsePolicyDecision *response = WEBKIT_RESPONSE_POLICY_DECISION(decision);
        const char *uri = webkit_uri_request_get_uri(webkit_response_policy_decision_get_request(response));
        if (r->acknowledged && !r->failed && mm_request_allowed(uri, r->doc->token, r->doc->resources->len) &&
            webkit_response_policy_decision_is_mime_type_supported(response)) {
            webkit_policy_decision_use(decision); return TRUE;
        }
    }
    webkit_policy_decision_ignore(decision); return TRUE;
}
static void load_changed(WebKitWebView *view, WebKitLoadEvent event, gpointer data) {
    Reader *r = data;
    if (event == WEBKIT_LOAD_FINISHED && r->loading && r->acknowledged && !r->failed && !r->ready &&
        !g_strcmp0(webkit_web_view_get_uri(view), r->doc->document_uri)) {
        r->ready = TRUE;
        gtk_label_set_text(GTK_LABEL(r->notice), "Remote resources blocked. This is not the inline reader.");
        if (r->deadline) { g_source_remove(r->deadline); r->deadline = 0; }
        mm_emit("ready", r->doc->token, "policy", MM_READY_POLICY);
        gtk_stack_set_visible_child_name(GTK_STACK(r->stack),
            gtk_toggle_button_get_active(GTK_TOGGLE_BUTTON(r->plain_button)) ? "plain" : "original");
    }
}
static gboolean load_failed(WebKitWebView *view, WebKitLoadEvent event, const char *uri, GError *error, gpointer data) {
    (void)view; (void)event; (void)uri; (void)error;
    Reader *r = data;
    /* Bootstrap cancellation when replacing about:blank is harmless. Never
     * persist WebKit's error text: it can include message URLs. */
    if (!r->loading) failed(r, "bootstrap-failed");
    else if (!r->ready && !g_strcmp0(uri, r->doc->document_uri)) failed(r, "document-load-failed");
    return TRUE;
}
static void terminated(WebKitWebView *view, WebKitWebProcessTerminationReason reason, gpointer data) {
    (void)view; (void)reason; failed(data, "web-process-terminated");
}
static gboolean deny_permission(WebKitWebView *view, WebKitPermissionRequest *request, gpointer unused) {
    (void)view; (void)unused; webkit_permission_request_deny(request); return TRUE;
}
static gboolean deny_context_menu(WebKitWebView *view, WebKitContextMenu *menu, GdkEvent *event, WebKitHitTestResult *hit, gpointer unused) {
    (void)view; (void)menu; (void)event; (void)hit; (void)unused;
    /* Default menus offer navigation/download/inspect actions. Ctrl+C remains
     * the real WebKit selection-copy command; no JS bridge is involved. */
    return TRUE;
}
static GtkWidget *deny_popup(WebKitWebView *view, WebKitNavigationAction *action, gpointer unused) {
    (void)view; (void)action; (void)unused; return NULL;
}
static void deny_download(WebKitWebContext *context, WebKitDownload *download, gpointer unused) {
    (void)context; (void)unused; webkit_download_cancel(download);
}
static gboolean deny_dialog(WebKitWebView *view, WebKitScriptDialog *dialog, gpointer unused) {
    (void)view; (void)unused; webkit_script_dialog_close(dialog); return TRUE;
}
static gboolean deny_auth(WebKitWebView *view, WebKitAuthenticationRequest *request, gpointer unused) {
    (void)view; (void)unused; webkit_authentication_request_cancel(request); return TRUE;
}
static void plain_toggled(GtkToggleButton *button, gpointer data) {
    Reader *r = data;
    gtk_stack_set_visible_child_name(GTK_STACK(r->stack),
        gtk_toggle_button_get_active(button) || !r->ready || r->failed ? "plain" : "original");
}
static void zoom(GtkButton *button, gpointer data) {
    Reader *r = data;
    const char *action = gtk_widget_get_name(GTK_WIDGET(button));
    double value = webkit_web_view_get_zoom_level(r->view);
    value = !strcmp(action, "reset") ? 1.0 : !strcmp(action, "in") ? MIN(3.0, value+0.1) : MAX(0.5, value-0.1);
    webkit_web_view_set_zoom_level(r->view, value);
}
static int policy_test(const char *wire, const char *token) {
    GError *error = NULL;
    MmDocument *doc = mm_document_parse(wire, strlen(wire), token, &error);
    if (!doc) { mm_emit("error", NULL, "code", "request-rejected"); g_clear_error(&error); return 2; }
    mm_emit("policy-document", token, "html", doc->html);
    mm_document_free(doc); return 0;
}
int mm_policy_self_test(void);
int main(int argc, char **argv) {
    struct rlimit zero = {0, 0};
    if (setrlimit(RLIMIT_CORE, &zero) < 0) return 2;
    GError *error = NULL;
    if (argc == 2 && !strcmp(argv[1], "--self-test")) return mm_policy_self_test();
    gboolean test = argc == 2 && !strcmp(argv[1], "--policy-test");
    gboolean contained = argc == 2 && !strcmp(argv[1], "--contained");
    if (!test && !contained) {
        if (argc != 1 || !mm_enter_sandbox(argv[0], &error)) {
            mm_emit("error", NULL, "code", "containment-unavailable"); g_clear_error(&error); return 2;
        }
    }
    if (!test && !mm_verify_sandbox(&error)) {
        mm_emit("error", NULL, "code", "containment-unavailable"); g_clear_error(&error); return 2;
    }
    char *wire = mm_read_input(&error);
    if (!wire) { mm_emit("error", NULL, "code", "input-rejected"); g_clear_error(&error); return 2; }
    if (test) { int code = policy_test(wire, "0123456789abcdef0123456789abcdef"); g_free(wire); return code; }
    char *uuid = g_uuid_string_random();
    GString *token = g_string_new(NULL);
    for (const char *p = uuid; *p; ++p) if (*p != '-') g_string_append_c(token, *p);
    g_free(uuid);
    MmDocument *doc = mm_document_parse(wire, strlen(wire), token->str, &error);
    g_free(wire); g_string_free(token, TRUE);
    if (!doc) { mm_emit("error", NULL, "code", "request-rejected"); g_clear_error(&error); return 2; }
    if (!gtk_init_check(&argc, &argv)) { mm_emit("error", NULL, "code", "display-unavailable"); mm_document_free(doc); return 2; }
    Reader r = { .doc = doc };
    if (!g_file_test("/app/extensions/libmegamail-reader-policy.so", G_FILE_TEST_IS_REGULAR)) {
        mm_emit("error", NULL, "code", "extension-unavailable"); mm_document_free(doc); return 2;
    }
    WebKitWebContext *context = webkit_web_context_new_ephemeral();
    webkit_web_context_set_sandbox_enabled(context, TRUE);
    if (!webkit_web_context_get_sandbox_enabled(context) || !webkit_web_context_is_ephemeral(context)) {
        mm_emit("error", NULL, "code", "engine-policy-unavailable"); mm_document_free(doc); g_object_unref(context); return 2;
    }
    webkit_web_context_set_web_extensions_directory(context, "/app/extensions");
    webkit_web_context_set_web_extensions_initialization_user_data(context, g_variant_new("(su)", doc->token, doc->resources->len));
    webkit_web_context_register_uri_scheme(context, "megamail-resource", scheme_request, &r, NULL);
    WebKitSecurityManager *security = webkit_web_context_get_security_manager(context);
    webkit_security_manager_register_uri_scheme_as_secure(security, "megamail-resource");
    webkit_security_manager_register_uri_scheme_as_display_isolated(security, "megamail-resource");
    webkit_cookie_manager_set_accept_policy(webkit_web_context_get_cookie_manager(context), WEBKIT_COOKIE_POLICY_ACCEPT_NEVER);
    g_signal_connect(context, "download-started", G_CALLBACK(deny_download), NULL);
    WebKitSettings *settings = webkit_settings_new();
    webkit_settings_set_enable_javascript(settings, FALSE);
    webkit_settings_set_enable_javascript_markup(settings, FALSE);
    webkit_settings_set_enable_html5_local_storage(settings, FALSE);
    webkit_settings_set_enable_html5_database(settings, FALSE);
    webkit_settings_set_enable_offline_web_application_cache(settings, FALSE);
    webkit_settings_set_enable_page_cache(settings, FALSE);
    webkit_settings_set_enable_webgl(settings, FALSE);
    webkit_settings_set_enable_media(settings, FALSE);
    webkit_settings_set_enable_media_stream(settings, FALSE);
    webkit_settings_set_enable_webrtc(settings, FALSE);
    webkit_settings_set_enable_mediasource(settings, FALSE);
    webkit_settings_set_enable_encrypted_media(settings, FALSE);
    webkit_settings_set_enable_developer_extras(settings, FALSE);
    webkit_settings_set_enable_dns_prefetching(settings, FALSE);
    webkit_settings_set_javascript_can_open_windows_automatically(settings, FALSE);
    webkit_settings_set_allow_file_access_from_file_urls(settings, FALSE);
    webkit_settings_set_allow_universal_access_from_file_urls(settings, FALSE);
    webkit_settings_set_allow_top_navigation_to_data_urls(settings, FALSE);
    webkit_settings_set_enable_fullscreen(settings, FALSE);
    webkit_settings_set_hardware_acceleration_policy(settings, WEBKIT_HARDWARE_ACCELERATION_POLICY_NEVER);
    r.view = g_object_new(WEBKIT_TYPE_WEB_VIEW, "web-context", context, "settings", settings, NULL);
    g_object_unref(settings);
    GdkRGBA white = {1, 1, 1, 1}; webkit_web_view_set_background_color(r.view, &white);
    g_signal_connect(r.view, "user-message-received", G_CALLBACK(policy_message), &r);
    g_signal_connect(r.view, "decide-policy", G_CALLBACK(decide_policy), &r);
    g_signal_connect(r.view, "load-changed", G_CALLBACK(load_changed), &r);
    g_signal_connect(r.view, "load-failed", G_CALLBACK(load_failed), &r);
    g_signal_connect(r.view, "web-process-terminated", G_CALLBACK(terminated), &r);
    g_signal_connect(r.view, "permission-request", G_CALLBACK(deny_permission), NULL);
    g_signal_connect(r.view, "context-menu", G_CALLBACK(deny_context_menu), NULL);
    g_signal_connect(r.view, "create", G_CALLBACK(deny_popup), NULL);
    g_signal_connect(r.view, "script-dialog", G_CALLBACK(deny_dialog), NULL);
    g_signal_connect(r.view, "authenticate", G_CALLBACK(deny_auth), NULL);
    r.window = gtk_window_new(GTK_WINDOW_TOPLEVEL);
    gtk_window_set_title(GTK_WINDOW(r.window), "Original formatting — temporary window");
    gtk_window_set_default_size(GTK_WINDOW(r.window), 680, 600);
    GtkWidget *box = gtk_box_new(GTK_ORIENTATION_VERTICAL, 0); gtk_container_add(GTK_CONTAINER(r.window), box);
    /* Horizontal GtkBox minimums add: the old four full-label buttons plus
     * spacing/border match the observed 455px fixture clamp. Separate view/close
     * from zoom rather than forcing document CSS, zoom or geometry hints. */
    GtkWidget *toolbar = gtk_box_new(GTK_ORIENTATION_VERTICAL, 8);
    gtk_container_set_border_width(GTK_CONTAINER(toolbar), 12); gtk_box_pack_start(GTK_BOX(box), toolbar, FALSE, FALSE, 0);
    GtkWidget *view_actions = gtk_box_new(GTK_ORIENTATION_HORIZONTAL, 8);
    GtkWidget *zoom_actions = gtk_box_new(GTK_ORIENTATION_HORIZONTAL, 8);
    gtk_box_pack_start(GTK_BOX(toolbar), view_actions, FALSE, FALSE, 0);
    gtk_box_pack_start(GTK_BOX(toolbar), zoom_actions, FALSE, FALSE, 0);
    GtkWidget *plain = gtk_toggle_button_new_with_mnemonic("_Plain text");
    r.plain_button = plain;
    gtk_box_pack_start(GTK_BOX(view_actions), plain, FALSE, FALSE, 0);
    g_signal_connect(plain, "toggled", G_CALLBACK(plain_toggled), &r);
    GtkWidget *close = gtk_button_new_with_mnemonic("_Close");
    gtk_box_pack_end(GTK_BOX(view_actions), close, FALSE, FALSE, 0);
    g_signal_connect_swapped(close, "clicked", G_CALLBACK(gtk_window_close), r.window);
    GtkAccelGroup *accelerators = gtk_accel_group_new();
    gtk_window_add_accel_group(GTK_WINDOW(r.window), accelerators);
    gtk_widget_add_accelerator(close, "clicked", accelerators, GDK_KEY_w, GDK_CONTROL_MASK, GTK_ACCEL_VISIBLE);
    gtk_widget_set_tooltip_text(close, "Close temporary window (Ctrl+W)");
    g_object_unref(accelerators);
    const char *actions[] = {"out", "reset", "in"}; const char *labels[] = {"Zoom _out", "_Reset", "Zoom _in"};
    const char *zoom_names[] = {"Zoom out", "Reset zoom to 100%", "Zoom in"};
    for (guint i = 0; i < 3; ++i) {
        GtkWidget *button = gtk_button_new_with_mnemonic(labels[i]); gtk_widget_set_name(button, actions[i]);
        gtk_widget_set_tooltip_text(button, zoom_names[i]);
        atk_object_set_name(gtk_widget_get_accessible(button), zoom_names[i]);
        gtk_box_pack_start(GTK_BOX(zoom_actions), button, FALSE, FALSE, 0); g_signal_connect(button, "clicked", G_CALLBACK(zoom), &r);
    }
    GtkWidget *subject = gtk_label_new(doc->title);
    gtk_label_set_selectable(GTK_LABEL(subject), TRUE);
    /* Width chars is the minimum, max width chars the natural request only;
     * wrapped labels still use all allocated width. Keep every character,
     * including long unbroken titles, without imposing a horizontal minimum. */
    gtk_label_set_line_wrap(GTK_LABEL(subject), TRUE);
    gtk_label_set_line_wrap_mode(GTK_LABEL(subject), PANGO_WRAP_WORD_CHAR);
    gtk_label_set_width_chars(GTK_LABEL(subject), 1);
    gtk_label_set_max_width_chars(GTK_LABEL(subject), 60);
    gtk_widget_set_hexpand(subject, TRUE);
    gtk_widget_set_margin_start(subject, 12); gtk_widget_set_margin_end(subject, 12);
    gtk_box_pack_start(GTK_BOX(box), subject, FALSE, FALSE, 0);
    GtkWidget *notice = gtk_label_new("Loading original formatting. Remote resources remain blocked.");
    r.notice = notice;
    gtk_label_set_line_wrap(GTK_LABEL(notice), TRUE);
    gtk_label_set_line_wrap_mode(GTK_LABEL(notice), PANGO_WRAP_WORD_CHAR);
    gtk_label_set_width_chars(GTK_LABEL(notice), 1);
    gtk_label_set_max_width_chars(GTK_LABEL(notice), 60);
    gtk_widget_set_hexpand(notice, TRUE);
    gtk_widget_set_margin_start(notice, 12); gtk_widget_set_margin_end(notice, 12);
    gtk_widget_set_margin_bottom(notice, 8);
    gtk_box_pack_start(GTK_BOX(box), notice, FALSE, FALSE, 0);
    r.stack = gtk_stack_new(); gtk_box_pack_start(GTK_BOX(box), r.stack, TRUE, TRUE, 0);
    GtkWidget *scroll = gtk_scrolled_window_new(NULL, NULL); GtkWidget *text = gtk_text_view_new();
    gtk_text_view_set_editable(GTK_TEXT_VIEW(text), FALSE); gtk_text_view_set_wrap_mode(GTK_TEXT_VIEW(text), GTK_WRAP_WORD_CHAR);
    gtk_text_view_set_left_margin(GTK_TEXT_VIEW(text), 16); gtk_text_view_set_right_margin(GTK_TEXT_VIEW(text), 16);
    gtk_text_buffer_set_text(gtk_text_view_get_buffer(GTK_TEXT_VIEW(text)),
        *doc->plain ? doc->plain : "No plain-text alternative was supplied.", -1);
    gtk_container_add(GTK_CONTAINER(scroll), text);
    gtk_stack_add_named(GTK_STACK(r.stack), scroll, "plain");
    gtk_stack_add_named(GTK_STACK(r.stack), GTK_WIDGET(r.view), "original");
    gtk_stack_set_visible_child_name(GTK_STACK(r.stack), "plain");
    g_signal_connect(r.window, "destroy", G_CALLBACK(gtk_main_quit), NULL);
    gtk_widget_show_all(r.window);
    r.deadline = g_timeout_add_seconds(10, deadline, &r);
    guint pulse = g_timeout_add_seconds(2, heartbeat, &r);
    /* Only trusted about:blank bootstraps the extension. No untrusted load until ack. */
    webkit_web_view_load_uri(r.view, "about:blank");
    gtk_main();
    if (r.deadline) g_source_remove(r.deadline);
    g_source_remove(pulse);
    mm_emit("closed", doc->token, NULL, NULL);
    g_object_unref(context); mm_document_free(doc);
    return r.failed ? 2 : 0;
}
