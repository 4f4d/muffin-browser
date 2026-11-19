#include <gtk/gtk.h>
#include <webkit2/webkit2.h>
#include <stdio.h>

static int result = 1;

static void finished(WebKitWebView *view, WebKitLoadEvent event, gpointer data) {
    (void)data;
    if (event != WEBKIT_LOAD_FINISHED) {
        return;
    }
    const char *uri = webkit_web_view_get_uri(view);
    printf("uri=%s\nuser_agent=%s\n", uri,
           webkit_settings_get_user_agent(webkit_web_view_get_settings(view)));
    result = g_strcmp0(uri, "about:blank") == 0 ? 0 : 1;
    gtk_main_quit();
}

static gboolean timed_out(gpointer data) {
    (void)data;
    fputs("about:blank did not finish within the watchdog interval\n", stderr);
    gtk_main_quit();
    return G_SOURCE_REMOVE;
}

int main(int argc, char **argv) {
    if (!gtk_init_check(&argc, &argv)) {
        fputs("GTK could not initialize the requested backend\n", stderr);
        return 1;
    }
    printf("gtk=%d.%d.%d\nwebkitgtk=%d.%d.%d\nbackend=%s\n",
           gtk_get_major_version(), gtk_get_minor_version(), gtk_get_micro_version(),
           webkit_get_major_version(), webkit_get_minor_version(), webkit_get_micro_version(),
           G_OBJECT_TYPE_NAME(gdk_display_get_default()));
    GtkWidget *window = gtk_window_new(GTK_WINDOW_TOPLEVEL);
    gtk_window_set_default_size(GTK_WINDOW(window), 800, 600);
    WebKitWebContext *context = webkit_web_context_new_ephemeral();
    GtkWidget *view = webkit_web_view_new_with_context(context);
    g_object_unref(context);
    gtk_container_add(GTK_CONTAINER(window), view);
    g_signal_connect(view, "load-changed", G_CALLBACK(finished), NULL);
    gtk_widget_show_all(window);
    guint timer = g_timeout_add_seconds(25, timed_out, NULL);
    webkit_web_view_load_uri(WEBKIT_WEB_VIEW(view), "about:blank");
    gtk_main();
    if (result == 0) {
        g_source_remove(timer);
    }
    gtk_widget_destroy(window);
    return result;
}
