/* Local WebKitGTK control for the S4 native resource and PDF probes. */
#include <gtk/gtk.h>
#include <webkit2/webkit2.h>

typedef struct {
    const gchar *base;
    const gchar *output;
    gboolean filtered;
    gboolean pdf;
    gint status;
    gint64 start;
    WebKitWebView *view;
    WebKitUserContentManager *manager;
} Probe;

static gboolean timed_out(gpointer data) {
    Probe *probe = data;
    probe->status = 2;
    g_print("engine_probe=timeout\n");
    gtk_main_quit();
    return G_SOURCE_REMOVE;
}

static void snapshot_finished(GObject *object, GAsyncResult *result, gpointer data) {
    Probe *probe = data;
    GError *error = NULL;
    cairo_surface_t *surface = webkit_web_view_get_snapshot_finish(WEBKIT_WEB_VIEW(object), result, &error);
    if (surface) {
        gchar *path = g_build_filename(probe->output, "pdf-view.png", NULL);
        cairo_status_t status = cairo_surface_write_to_png(surface, path);
        g_print("pdf_snapshot=%s\n", status == CAIRO_STATUS_SUCCESS ? "true" : "false");
        g_free(path);
        cairo_surface_destroy(surface);
    } else {
        g_print("pdf_snapshot_error=%s\n", error->message);
        g_clear_error(&error);
    }
    gtk_main_quit();
}

static gboolean take_snapshot(gpointer data) {
    Probe *probe = data;
    webkit_web_view_get_snapshot(probe->view, WEBKIT_SNAPSHOT_REGION_VISIBLE,
                                WEBKIT_SNAPSHOT_OPTIONS_NONE, NULL, snapshot_finished, probe);
    return G_SOURCE_REMOVE;
}

static gboolean decide_policy(WebKitWebView *view, WebKitPolicyDecision *decision,
                              WebKitPolicyDecisionType type, gpointer data) {
    Probe *probe = data;
    (void)view;
    if (probe->pdf && type == WEBKIT_POLICY_DECISION_TYPE_RESPONSE) {
        WebKitResponsePolicyDecision *response = WEBKIT_RESPONSE_POLICY_DECISION(decision);
        if (webkit_response_policy_decision_is_main_frame_main_resource(response)) {
            g_print("pdf_mime_supported=%s\n",
                    webkit_response_policy_decision_is_mime_type_supported(response) ? "true" : "false");
        }
    }
    return FALSE;
}

static void load_changed(WebKitWebView *view, WebKitLoadEvent event, gpointer data) {
    Probe *probe = data;
    if (event != WEBKIT_LOAD_FINISHED) return;
    if (probe->pdf) {
        g_timeout_add(900, take_snapshot, probe);
        return;
    }
    g_print("images_finished elapsed_ms=%.3f\n", (g_get_monotonic_time() - probe->start) / 1000.0);
    if (probe->filtered) {
        probe->pdf = TRUE;
        gchar *url = g_strconcat(probe->base, "/sample.pdf", NULL);
        webkit_web_view_load_uri(view, url);
        g_free(url);
    } else {
        gtk_main_quit();
    }
}

static void launch(Probe *probe) {
    WebKitWebContext *context = webkit_web_context_new_ephemeral();
    probe->view = WEBKIT_WEB_VIEW(g_object_new(
        WEBKIT_TYPE_WEB_VIEW, "web-context", context, "user-content-manager", probe->manager, NULL
    ));
    GtkWidget *window = gtk_window_new(GTK_WINDOW_TOPLEVEL);
    gtk_window_set_default_size(GTK_WINDOW(window), 620, 450);
    gtk_container_add(GTK_CONTAINER(window), GTK_WIDGET(probe->view));
    g_signal_connect(probe->view, "load-changed", G_CALLBACK(load_changed), probe);
    g_signal_connect(probe->view, "decide-policy", G_CALLBACK(decide_policy), probe);
    gtk_widget_show_all(window);
    gchar *url = g_strconcat(probe->base, "/images", NULL);
    probe->start = g_get_monotonic_time();
    webkit_web_view_load_uri(probe->view, url);
    g_free(url);
    g_object_unref(context);
}

static void filter_finished(GObject *object, GAsyncResult *result, gpointer data) {
    Probe *probe = data;
    GError *error = NULL;
    WebKitUserContentFilter *filter =
        webkit_user_content_filter_store_save_finish(WEBKIT_USER_CONTENT_FILTER_STORE(object), result, &error);
    if (!filter) {
        g_print("rule_compile_error=%s\n", error->message);
        g_clear_error(&error);
        probe->status = 3;
        gtk_main_quit();
        return;
    }
    g_print("rule_compiled=true elapsed_ms=%.3f\n", (g_get_monotonic_time() - probe->start) / 1000.0);
    webkit_user_content_manager_add_filter(probe->manager, filter);
    webkit_user_content_filter_unref(filter);
    launch(probe);
}

int main(int argc, char **argv) {
    if (argc != 4 || !g_str_has_prefix(argv[1], "http://127.0.0.1:") || !gtk_init_check(NULL, NULL))
        return 1;
    Probe probe = {.base = argv[1], .output = argv[3], .filtered = g_str_equal(argv[2], "filtered")};
    probe.manager = webkit_user_content_manager_new();
    g_print("engine=%u.%u.%u\n", webkit_get_major_version(), webkit_get_minor_version(), webkit_get_micro_version());
    g_timeout_add_seconds(18, timed_out, &probe);
    if (probe.filtered) {
        const gchar rules[] = "[{\"trigger\":{\"url-filter\":\"/blocked[.]svg$\"},\"action\":{\"type\":\"block\"}}]";
        gchar *rule_path = g_build_filename(probe.output, "rules", NULL);
        WebKitUserContentFilterStore *store = webkit_user_content_filter_store_new(rule_path);
        GBytes *source = g_bytes_new_static(rules, sizeof(rules) - 1);
        probe.start = g_get_monotonic_time();
        webkit_user_content_filter_store_save(store, "muffin-test", source, NULL, filter_finished, &probe);
        g_bytes_unref(source);
        g_object_unref(store);
        g_free(rule_path);
    } else {
        launch(&probe);
    }
    gtk_main();
    g_object_unref(probe.manager);
    return probe.status;
}
