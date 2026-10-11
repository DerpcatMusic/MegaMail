/* SPDX-License-Identifier: AGPL-3.0-or-later */
#define _GNU_SOURCE
#include "sandbox.h"
#include <sys/stat.h>
#include <fcntl.h>
#include <unistd.h>
#include <stdlib.h>
#include <string.h>

static gboolean fail(GError **error) {
    g_set_error_literal(error, g_quark_from_static_string("megamail-reader"), 1, "containment-unavailable");
    return FALSE;
}
static void arg(GPtrArray *args, const char *value) { g_ptr_array_add(args, g_strdup(value)); }
static void ro_bind(GPtrArray *args, const char *source, const char *target) {
    arg(args, "--ro-bind"); arg(args, source); arg(args, target);
}
static gboolean enter(const char *self, gboolean snapshot, GError **error) {
    (void)self;
    char executable[4096]; ssize_t size = readlink("/proc/self/exe", executable, sizeof executable-1);
    if (size < 0 || size >= (ssize_t)sizeof executable-1) return fail(error);
    executable[size] = 0;
    char *package = g_path_get_dirname(executable);
    /* Keep the original namespace identity on fd 3. The inner helper verifies
     * bubblewrap really changed it; an environment flag is not sufficient. */
    int fd = open("/proc/self/ns/net", O_RDONLY);
    if (fd < 0 || (fd != 3 && dup2(fd, 3) < 0)) { g_free(package); if (fd >= 0) close(fd); return fail(error); }
    if (fd != 3) close(fd);
    if (fcntl(3, F_SETFD, 0) < 0) { g_free(package); close(3); return fail(error); }
    GPtrArray *args = g_ptr_array_new_with_free_func(g_free);
    arg(args, "/usr/bin/bwrap");
    const char *fixed[] = { "--die-with-parent", "--new-session", "--unshare-user", "--unshare-pid",
        "--unshare-net", "--unshare-ipc", "--unshare-uts", "--cap-drop", "ALL",
        "--clearenv", "--proc", "/proc", "--dev", "/dev",
        /* WebKit's inner sandbox binds these paths unconditionally. Supply
         * private empty directories, never host sysfs or device metadata. */
        "--tmpfs", "/sys", "--dir", "/sys/block", "--dir", "/sys/bus",
        "--dir", "/sys/class", "--dir", "/sys/dev", "--dir", "/sys/devices",
        "--remount-ro", "/sys",
        "--tmpfs", "/tmp", "--tmpfs", "/run", "--tmpfs", "/home", "--dir", "/home/reader",
        "--setenv", "HOME", "/home/reader", "--setenv", "PATH", "/usr/bin",
        "--setenv", "NO_AT_BRIDGE", "1", "--setenv", "LANG", "C.UTF-8",
        "--chdir", "/home/reader" };
    for (guint i = 0; i < G_N_ELEMENTS(fixed); ++i) arg(args, fixed[i]);
    ro_bind(args, "/usr", "/usr");
    for (guint i = 0; i < 2; ++i) {
        const char *p = i ? "/lib64" : "/lib";
        if (g_file_test(p, G_FILE_TEST_EXISTS)) ro_bind(args, p, p);
    }
    ro_bind(args, "/etc/fonts", "/etc/fonts");
    if (g_file_test("/etc/ld.so.cache", G_FILE_TEST_EXISTS)) ro_bind(args, "/etc/ld.so.cache", "/etc/ld.so.cache");
    ro_bind(args, package, "/app"); g_free(package);
    const char *backend = g_getenv("GDK_BACKEND");
    const char *wayland = g_getenv("WAYLAND_DISPLAY");
    gboolean is_wayland = (!backend || !strcmp(backend, "wayland")) && wayland;
    if (snapshot) {
        arg(args, "--symlink"); arg(args, "usr/bin"); arg(args, "/bin");
        arg(args, "--dir"); arg(args, "/var"); arg(args, "--dir"); arg(args, "/var/lib"); arg(args, "--dir"); arg(args, "/var/lib/xkb");
        arg(args, "--setenv"); arg(args, "GDK_BACKEND"); arg(args, "x11");
        arg(args, "--setenv"); arg(args, "DISPLAY"); arg(args, ":90");
    } else if (is_wayland) {
        const char *runtime = g_getenv("XDG_RUNTIME_DIR");
        if (!runtime || !g_path_is_absolute(runtime) || !*wayland || strlen(wayland) > 128 ||
            !strcmp(wayland, ".") || !strcmp(wayland, "..") || strchr(wayland, '/')) goto denied;
        char *socket = g_build_filename(runtime, wayland, NULL);
        struct stat st;
        gboolean ok = lstat(socket, &st) == 0 && S_ISSOCK(st.st_mode) && st.st_uid == getuid();
        if (!ok) { g_free(socket); goto denied; }
        char *target = g_build_filename("/run/reader", wayland, NULL);
        arg(args, "--dir"); arg(args, "/run/reader");
        ro_bind(args, socket, target); g_free(socket); g_free(target);
        arg(args, "--setenv"); arg(args, "XDG_RUNTIME_DIR"); arg(args, "/run/reader");
        arg(args, "--setenv"); arg(args, "WAYLAND_DISPLAY"); arg(args, wayland);
        arg(args, "--setenv"); arg(args, "GDK_BACKEND"); arg(args, "wayland");
    } else {
        /* Local X socket only. Never mount .Xauthority/home credentials. Typical
         * authenticated production X11 needs a separately reviewed display grant.
         * Synthetic Xvfb -ac works; otherwise fail back to native plain text. */
        const char *display = g_getenv("DISPLAY");
        if (!display || display[0] != ':' || !display[1] || strlen(display) > 16) goto denied;
        for (const char *p = display+1; *p; ++p) if (!g_ascii_isdigit(*p)) goto denied;
        char *socket = g_strdup_printf("/tmp/.X11-unix/X%s", display+1);
        struct stat st; gboolean ok = lstat(socket, &st) == 0 && S_ISSOCK(st.st_mode);
        if (!ok) { g_free(socket); goto denied; }
        arg(args, "--dir"); arg(args, "/tmp/.X11-unix"); ro_bind(args, socket, socket); g_free(socket);
        arg(args, "--setenv"); arg(args, "DISPLAY"); arg(args, display);
        arg(args, "--setenv"); arg(args, "GDK_BACKEND"); arg(args, "x11");
    }
    arg(args, "/app/megamail-html-reader"); arg(args, snapshot ? "--contained-snapshot" : "--contained");
    g_ptr_array_add(args, NULL);
    execv("/usr/bin/bwrap", (char *const *)args->pdata);
 denied:
    g_ptr_array_unref(args); close(3); return fail(error);
}
gboolean mm_enter_sandbox(const char *self, GError **error) { return enter(self, FALSE, error); }
gboolean mm_enter_snapshot(const char *self, GError **error) { return enter(self, TRUE, error); }
gboolean mm_verify_sandbox(GError **error) {
    struct stat original, current;
    int fd = open("/proc/self/ns/net", O_RDONLY | O_CLOEXEC);
    gboolean ok = fd >= 0 && fstat(3, &original) == 0 && fstat(fd, &current) == 0 &&
        (original.st_dev != current.st_dev || original.st_ino != current.st_ino);
    if (fd >= 0) close(fd);
    close(3);
    return ok ? TRUE : fail(error);
}
