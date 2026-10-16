/* SPDX-License-Identifier: AGPL-3.0-or-later */
#include "policy.h"
#include <gdk-pixbuf/gdk-pixbuf.h>
#include <libxml/HTMLparser.h>
#include <libxml/HTMLtree.h>
#include <poll.h>
#include <unistd.h>
#include <errno.h>
#include <stdio.h>
#include <string.h>

static gboolean fail(GError **error, const char *code) {
    g_set_error_literal(error, g_quark_from_static_string("megamail-reader"), 1, code);
    return FALSE;
}
static gboolean listed(const char *name, const char *list) {
    char *needle = g_strdup_printf("|%s|", name);
    gboolean result = strstr(list, needle) != NULL;
    g_free(needle);
    return result;
}
static gboolean only_members(JsonObject *obj, const char *names) {
    GList *members = json_object_get_members(obj);
    gboolean ok = TRUE;
    for (GList *p = members; p; p = p->next) if (!listed(p->data, names)) ok = FALSE;
    g_list_free(members);
    return ok;
}
static const char *string_member(JsonObject *obj, const char *key, gsize max) {
    JsonNode *n = json_object_get_member(obj, key);
    if (!n || !JSON_NODE_HOLDS_VALUE(n) || json_node_get_value_type(n) != G_TYPE_STRING) return NULL;
    const char *s = json_node_get_string(n);
    if (!s || strlen(s) > max || !g_utf8_validate(s, -1, NULL)) return NULL;
    return s;
}
static char *normalize_cid(const char *s) {
    if (!s || !*s || strlen(s) > 256) return NULL;
    char *id = g_strdup(s);
    g_strstrip(id);
    gsize n = strlen(id);
    if (n >= 2 && id[0] == '<' && id[n-1] == '>') {
        memmove(id, id + 1, n - 2); id[n-2] = 0;
    }
    if (!*id) { g_free(id); return NULL; }
    for (char *p = id; *p; ++p) {
        if (!(g_ascii_isalnum(*p) || strchr("@._+-", *p))) { g_free(id); return NULL; }
        *p = g_ascii_tolower(*p);
    }
    return id;
}

static void resource_free(gpointer data) {
    MmResource *r = data;
    g_free(r->cid); g_free(r->uri); g_bytes_unref(r->png); g_free(r);
}
void mm_document_free(MmDocument *d) {
    if (!d) return;
    g_free(d->html); g_free(d->plain); g_free(d->title);
    g_free(d->token); g_free(d->document_uri);
    if (d->resources) g_ptr_array_unref(d->resources);
    g_free(d);
}

typedef struct { gboolean seen, oversized; guint64 pixels; } Dimensions;
static void size_prepared(GdkPixbufLoader *loader, int width, int height, gpointer data) {
    Dimensions *d = data;
    d->seen = TRUE;
    d->pixels = width > 0 && height > 0 ? (guint64)width * (guint64)height : G_MAXUINT64;
    d->oversized = width > 8192 || height > 8192 || d->pixels > MM_PIXELS_MAX;
    /* Stop oversized decode allocation at the first header. The caller stops
     * feeding chunks as soon as this flag is visible; no resized bomb is accepted. */
    if (d->oversized) gdk_pixbuf_loader_set_size(loader, 1, 1);
}
static GBytes *decode_raster(const char *mime, const char *encoded, guint64 *pixels, GError **error) {
    const char *type = !strcmp(mime, "image/png") ? "png" :
                       !strcmp(mime, "image/jpeg") ? "jpeg" :
                       !strcmp(mime, "image/webp") ? "webp" : NULL;
    gsize n = strlen(encoded);
    if (!type || !n || n % 4 || n > ((MM_RESOURCE_MAX + 2u) / 3u) * 4u) {
        fail(error, "resource-type-or-size"); return NULL;
    }
    /* Canonical RFC4648 only; GLib's permissive decoder alone is not a validator. */
    for (gsize i = 0; i < n; ++i) {
        if (!(g_ascii_isalnum(encoded[i]) || encoded[i] == '+' || encoded[i] == '/' ||
              (encoded[i] == '=' && i >= n-2))) { fail(error, "resource-base64"); return NULL; }
    }
    gsize size = 0;
    guchar *raw = g_base64_decode(encoded, &size);
    char *roundtrip = g_base64_encode(raw, size);
    gboolean canonical = !strcmp(encoded, roundtrip);
    g_free(roundtrip);
    if (!canonical || size > MM_RESOURCE_MAX) { g_free(raw); fail(error, "resource-base64"); return NULL; }
    GdkPixbufLoader *loader = gdk_pixbuf_loader_new_with_type(type, error);
    if (!loader) { g_free(raw); return NULL; }
    Dimensions dim = {0};
    g_signal_connect(loader, "size-prepared", G_CALLBACK(size_prepared), &dim);
    gboolean ok = TRUE;
    for (gsize off = 0; off < size && ok && !dim.oversized; off += 1024)
        ok = gdk_pixbuf_loader_write(loader, raw + off, MIN((gsize)1024, size-off), error);
    g_free(raw);
    if (dim.oversized) {
        ok = FALSE;
        if (!error || !*error) fail(error, "resource-pixels");
    }
    if (ok) ok = gdk_pixbuf_loader_close(loader, error);
    else gdk_pixbuf_loader_close(loader, NULL);
    if (ok && (!dim.seen || gdk_pixbuf_loader_get_animation(loader) == NULL ||
        !gdk_pixbuf_animation_is_static_image(gdk_pixbuf_loader_get_animation(loader)))) {
        ok = FALSE; fail(error, "resource-animation-or-header");
    }
    char *png = NULL; gsize png_size = 0;
    if (ok) ok = gdk_pixbuf_save_to_buffer(gdk_pixbuf_loader_get_pixbuf(loader), &png, &png_size, "png", error, NULL);
    g_object_unref(loader);
    if (ok && png_size > 4u * 1024u * 1024u) { ok = FALSE; fail(error, "resource-normalized-size"); }
    if (!ok) { g_free(png); return NULL; }
    *pixels = dim.pixels;
    return g_bytes_new_take(png, png_size);
}
static gboolean read_resources(MmDocument *d, JsonObject *root, GError **error) {
    JsonNode *n = json_object_get_member(root, "resources");
    if (!n) return TRUE;
    if (!JSON_NODE_HOLDS_ARRAY(n)) return fail(error, "resources-schema");
    JsonArray *array = json_node_get_array(n);
    guint count = json_array_get_length(array);
    if (count > MM_RESOURCES_MAX) return fail(error, "resources-count");
    guint64 pixels = 0; gsize normalized = 0, compressed = 0;
    for (guint i = 0; i < count; ++i) {
        JsonNode *item = json_array_get_element(array, i);
        if (!JSON_NODE_HOLDS_OBJECT(item)) return fail(error, "resource-schema");
        JsonObject *obj = json_node_get_object(item);
        const char *cid = string_member(obj, "cid", 256);
        const char *mime = string_member(obj, "mime", 32);
        const char *b64 = string_member(obj, "base64", ((MM_RESOURCE_MAX+2u)/3u)*4u);
        if (!only_members(obj, "|cid|mime|base64|") || !cid || !mime || !b64) return fail(error, "resource-schema");
        char *id = normalize_cid(cid);
        if (!id) return fail(error, "resource-cid");
        for (guint j = 0; j < d->resources->len; ++j) {
            MmResource *other = g_ptr_array_index(d->resources, j);
            if (!strcmp(id, other->cid)) { g_free(id); return fail(error, "resource-duplicate-cid"); }
        }
        compressed += strlen(b64) / 4u * 3u;
        if (compressed > 8u*1024u*1024u) { g_free(id); return fail(error, "resource-total-bytes"); }
        guint64 item_pixels = 0;
        GBytes *png = decode_raster(mime, b64, &item_pixels, error);
        if (!png) { g_free(id); return FALSE; }
        pixels += item_pixels; normalized += g_bytes_get_size(png);
        if (pixels > MM_TOTAL_PIXELS_MAX || normalized > MM_NORMALIZED_MAX) {
            g_free(id); g_bytes_unref(png); return fail(error, "resource-total-decoded");
        }
        MmResource *r = g_new0(MmResource, 1);
        r->cid = id; r->png = png;
        r->uri = g_strdup_printf("megamail-resource://%s/part/%u", d->token, i);
        g_ptr_array_add(d->resources, r);
    }
    return TRUE;
}

/* Reconstruct an inert tree, not string/regex deletion. libxml2's HTML parser is
 * not HTML5: mutation-XSS fixtures and CSP remain mandatory defense in depth.
 * CSS is retained verbatim as INERT layout input, NOT claimed to be sanitized.
 * External CSS resource tokens are blocked by CSP + extension + network namespace.
 * CSS cid/data images remain blocked in this slice; only img[src=cid] is rewritten. */
static gboolean copy_nodes(xmlNode *input, xmlNode *output, MmDocument *d, guint depth, guint *nodes) {
    if (depth > 128) return FALSE;
    for (xmlNode *n = input; n; n = n->next) {
        if (++*nodes > 50000) return FALSE;
        if (n->type == XML_TEXT_NODE) { xmlAddChild(output, xmlNewText(n->content)); continue; }
        if (n->type != XML_ELEMENT_NODE) continue;
        const char *tag = (const char *)n->name;
        if (n->ns || listed(tag, "|script|iframe|frame|frameset|object|embed|applet|svg|math|template|noscript|title|audio|video|source|track|link|meta|base|input|button|textarea|select|option|canvas|")) continue;
        if (!strcmp(tag, "html") || !strcmp(tag, "head") || !strcmp(tag, "form")) {
            if (!strcmp(tag, "html")) {
                for (xmlAttr *a = n->properties; a; a = a->next) {
                    if (!a->ns && listed((char *)a->name, "|class|id|style|dir|lang|")) {
                        xmlChar *value = xmlNodeListGetString(n->doc, a->children, 1);
                        if (value) xmlSetProp(output, a->name, value);
                        xmlFree(value);
                    }
                }
            }
            if (!copy_nodes(n->children, output, d, depth+1, nodes)) return FALSE;
            continue;
        }
        if (!listed(tag, "|body|style|div|span|p|a|img|table|thead|tbody|tfoot|tr|td|th|caption|colgroup|col|br|hr|h1|h2|h3|h4|h5|h6|b|strong|i|em|u|s|small|big|sub|sup|ul|ol|li|dl|dt|dd|blockquote|pre|code|center|font|section|article|header|footer|main|address|")) {
            if (!copy_nodes(n->children, output, d, depth+1, nodes)) return FALSE;
            continue;
        }
        /* Never serialize sender style as a raw-text node: escaped '<' prevents
         * a second HTML parser from interpreting an injected </style> sequence.
         * libxml2 serializes style text specially, so reject '<' in CSS entirely. */
        if (!strcmp(tag, "style")) {
            xmlChar *css = xmlNodeGetContent(n);
            if (css && !strchr((char *)css, '<')) {
                xmlNode *s = xmlNewChild(output, NULL, BAD_CAST "style", NULL);
                xmlAddChild(s, xmlNewText(css));
            }
            xmlFree(css); continue;
        }
        xmlNode *out = xmlNewChild(output, NULL, n->name, NULL);
        for (xmlAttr *a = n->properties; a; a = a->next) {
            if (a->ns) continue;
            const char *name = (const char *)a->name;
            xmlChar *value = xmlNodeListGetString(n->doc, a->children, 1);
            if (!value) continue;
            if (listed(name, "|class|id|style|dir|lang|title|align|valign|width|height|bgcolor|color|face|size|border|cellpadding|cellspacing|colspan|rowspan|scope|role|alt|start|type|"))
                xmlSetProp(out, a->name, value);
            else if (!strcmp(tag, "a") && !strcmp(name, "href") && mm_safe_link((char *)value))
                xmlSetProp(out, BAD_CAST "href", value);
            else if (!strcmp(tag, "img") && !strcmp(name, "src") && !g_ascii_strncasecmp((char *)value, "cid:", 4)) {
                char *decoded = g_uri_unescape_string((char *)value + 4, NULL);
                char *id = normalize_cid(decoded); g_free(decoded);
                for (guint i = 0; id && i < d->resources->len; ++i) {
                    MmResource *r = g_ptr_array_index(d->resources, i);
                    if (!strcmp(id, r->cid)) xmlSetProp(out, BAD_CAST "src", BAD_CAST r->uri);
                }
                g_free(id);
            }
            xmlFree(value);
        }
        if (!copy_nodes(n->children, out, d, depth+1, nodes)) return FALSE;
    }
    return TRUE;
}
static char *sanitize(const char *html, MmDocument *d, GError **error) {
    htmlDocPtr parsed = htmlReadMemory(html, (int)strlen(html), NULL, "UTF-8",
        HTML_PARSE_NONET | HTML_PARSE_NOERROR | HTML_PARSE_NOWARNING | HTML_PARSE_COMPACT);
    if (!parsed) { fail(error, "html-parse"); return NULL; }
    htmlDocPtr safe = htmlNewDocNoDtD(NULL, NULL);
    xmlNode *root = xmlNewNode(NULL, BAD_CAST "html"); xmlDocSetRootElement(safe, root);
    xmlNode *head = xmlNewChild(root, NULL, BAD_CAST "head", NULL);
    xmlNode *meta = xmlNewChild(head, NULL, BAD_CAST "meta", NULL);
    xmlSetProp(meta, BAD_CAST "charset", BAD_CAST "utf-8");
    meta = xmlNewChild(head, NULL, BAD_CAST "meta", NULL);
    xmlSetProp(meta, BAD_CAST "http-equiv", BAD_CAST "Content-Security-Policy");
    xmlSetProp(meta, BAD_CAST "content", BAD_CAST MM_CSP);
    /* Body author's declarations still win. Backing WebView pixels are white
     * even when sender CSS makes html/body transparent. No shell wallpaper. */
    xmlNewChild(head, NULL, BAD_CAST "style", BAD_CAST "html { background-color: white; } body { color: black; }");
    guint nodes = 0;
    gboolean ok = copy_nodes(xmlDocGetRootElement(parsed), root, d, 0, &nodes);
    xmlFreeDoc(parsed);
    xmlChar *buffer = NULL; int length = 0;
    if (ok) htmlDocDumpMemoryFormat(safe, &buffer, &length, 0);
    xmlFreeDoc(safe);
    if (!ok || !buffer || length < 0 || (guint)length > 4u*1024u*1024u) {
        xmlFree(buffer); fail(error, "html-tree-or-output-limit"); return NULL;
    }
    char *result = g_strndup((char *)buffer, length); xmlFree(buffer); return result;
}
/* Bound parser nesting and reject escaped NUL before GLib string APIs can
 * shorten a JSON value. The JSON parser still validates the full grammar. */
static gboolean bounded_json_strings(const char *wire, gsize length) {
    guint depth = 0; gboolean string = FALSE;
    for (gsize i = 0; i < length; ++i) {
        char c = wire[i];
        if (string && c == '\\') {
            if (++i >= length) return FALSE;
            if (wire[i] == 'u' && i + 4 < length && !memcmp(wire + i + 1, "0000", 4)) return FALSE;
            continue;
        }
        if (c == '"') { string = !string; continue; }
        if (!string && (c == '{' || c == '[') && ++depth > 16) return FALSE;
        if (!string && (c == '}' || c == ']')) { if (!depth) return FALSE; --depth; }
    }
    return !string && depth == 0;
}
MmDocument *mm_document_parse(const char *wire, gsize length, const char *token, GError **error) {
    if (!wire || length > MM_WIRE_MAX || memchr(wire, 0, length) || !g_utf8_validate(wire, length, NULL) ||
        !bounded_json_strings(wire, length)) {
        fail(error, "wire-size-or-encoding"); return NULL;
    }
    JsonParser *parser = json_parser_new();
    if (!json_parser_load_from_data(parser, wire, length, error)) { g_object_unref(parser); return NULL; }
    JsonNode *root = json_parser_get_root(parser);
    if (!root || !JSON_NODE_HOLDS_OBJECT(root)) { g_object_unref(parser); fail(error, "request-schema"); return NULL; }
    JsonObject *obj = json_node_get_object(root);
    JsonNode *version = json_object_get_member(obj, "version");
    const char *html = string_member(obj, "html", MM_SOURCE_MAX);
    const char *plain = string_member(obj, "plainText", MM_SOURCE_MAX);
    const char *title = string_member(obj, "title", 512);
    JsonNode *viewport = json_object_get_member(obj, "viewportWidth");
    if (!version || !JSON_NODE_HOLDS_VALUE(version) || json_node_get_value_type(version) != G_TYPE_INT64 ||
        json_node_get_int(version) != 1 || !html || !plain || !title ||
        !only_members(obj, "|version|html|plainText|title|resources|viewportWidth|deviceScale|") ||
        (viewport && (!JSON_NODE_HOLDS_VALUE(viewport) || json_node_get_value_type(viewport)!=G_TYPE_INT64 ||
         json_node_get_int(viewport)<320 || json_node_get_int(viewport)>1280))) {
        g_object_unref(parser); fail(error, "request-schema"); return NULL;
    }
    MmDocument *d = g_new0(MmDocument, 1);
    d->resources = g_ptr_array_new_with_free_func(resource_free);
    d->token = g_strdup(token); d->plain = g_strdup(plain); d->title = g_strdup(title);
    d->document_uri = g_strdup_printf("megamail-resource://%s/document", token);
    gboolean ok = read_resources(d, obj, error);
    if (ok) d->html = sanitize(html, d, error);
    g_object_unref(parser);
    if (!ok || !d->html) { mm_document_free(d); return NULL; }
    return d;
}

/* One EOF-delimited request, not a readline that can allocate an unbounded line.
 * Ten seconds total, including a producer that never closes stdin. */
char *mm_read_input(GError **error) {
    GString *wire = g_string_sized_new(4096);
    gint64 deadline = g_get_monotonic_time() + 10 * G_TIME_SPAN_SECOND;
    for (;;) {
        int ms = (int)MAX((gint64)0, (deadline - g_get_monotonic_time()) / 1000);
        struct pollfd fd = { STDIN_FILENO, POLLIN, 0 };
        int result = poll(&fd, 1, ms);
        if (result < 0 && errno == EINTR) continue;
        if (result <= 0) { fail(error, "input-timeout-or-io"); break; }
        char chunk[8192]; ssize_t n = read(STDIN_FILENO, chunk, sizeof chunk);
        if (n < 0 && errno == EINTR) continue;
        if (n < 0) { fail(error, "input-io"); break; }
        if (!n) return g_string_free(wire, FALSE);
        if (wire->len + (gsize)n > MM_WIRE_MAX) { fail(error, "wire-size"); break; }
        if (memchr(chunk, 0, (gsize)n)) { fail(error, "wire-nul"); break; }
        g_string_append_len(wire, chunk, n);
    }
    g_string_free(wire, TRUE); return NULL;
}
void mm_emit(const char *event, const char *token, const char *field, const char *value) {
    JsonBuilder *b = json_builder_new();
    json_builder_begin_object(b);
    json_builder_set_member_name(b, "version"); json_builder_add_int_value(b, 1);
    json_builder_set_member_name(b, "event"); json_builder_add_string_value(b, event);
    if (token) { json_builder_set_member_name(b, "token"); json_builder_add_string_value(b, token); }
    if (field) { json_builder_set_member_name(b, field); json_builder_add_string_value(b, value); }
    json_builder_end_object(b);
    JsonNode *root = json_builder_get_root(b); JsonGenerator *g = json_generator_new();
    json_generator_set_root(g, root); char *line = json_generator_to_data(g, NULL);
    puts(line); fflush(stdout);
    g_free(line); json_node_free(root); g_object_unref(g); g_object_unref(b);
}
