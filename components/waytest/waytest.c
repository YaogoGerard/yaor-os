/*
 * waytest — client Wayland de validation (ADR-024, TEST-GFX-001/002).
 *
 * Fenêtre xdg-shell rendue via wl_shm (XRGB8888) : rectangle bordé, horloge
 * digitale et compteurs d'événements pointeur/clavier affichés à l'écran.
 * Chaque événement d'entrée reçu est journalisé sur stderr — la preuve que
 * les entrées de la session (libinput sous le compositor) arrivent bien au
 * client (QA : injection QMP côté QEMU).
 *
 * Dépendances runtime : libwayland-client seule. Aucun moteur de rendu ni
 * bibliothèque de texte : police bitmap 5x7 intégrée (chiffres, ':', 'K',
 * 'P', espace). Sources internes du dépôt (« un besoin = un fournisseur » :
 * YAOR est son propre fournisseur pour la brique de validation).
 *
 * Connexion : WAYLAND_DISPLAY sous XDG_RUNTIME_DIR (socket du compositor).
 * Journal : stderr — ex. « waytest: K=1 P=42 key=30 press ».
 */

#include <errno.h>
#include <fcntl.h>
#include <signal.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/mman.h>
#include <sys/poll.h>
#include <sys/stat.h>
#include <sys/timerfd.h>
#include <time.h>
#include <unistd.h>

#include <wayland-client.h>
#include <wayland-client-protocol.h>
#include "xdg-shell-client-protocol.h"

/* ------------------------------------------------------------------ fonte */

/* 7 lignes de 5 colonnes (bit 4 = colonne la plus à gauche). */
static const uint8_t GLYPH[14][7] = {
    {0x0E, 0x11, 0x13, 0x15, 0x19, 0x11, 0x0E}, /* '0' */
    {0x04, 0x0C, 0x04, 0x04, 0x04, 0x04, 0x0E}, /* '1' */
    {0x0E, 0x11, 0x01, 0x02, 0x04, 0x08, 0x1F}, /* '2' */
    {0x1F, 0x02, 0x04, 0x02, 0x01, 0x11, 0x0E}, /* '3' */
    {0x02, 0x06, 0x0A, 0x12, 0x1F, 0x02, 0x02}, /* '4' */
    {0x1F, 0x10, 0x1E, 0x01, 0x01, 0x11, 0x0E}, /* '5' */
    {0x06, 0x08, 0x10, 0x1E, 0x11, 0x11, 0x0E}, /* '6' */
    {0x1F, 0x01, 0x02, 0x04, 0x08, 0x08, 0x08}, /* '7' */
    {0x0E, 0x11, 0x11, 0x0E, 0x11, 0x11, 0x0E}, /* '8' */
    {0x0E, 0x11, 0x11, 0x0F, 0x01, 0x02, 0x0C}, /* '9' */
    {0x00, 0x04, 0x00, 0x00, 0x00, 0x04, 0x00}, /* ':' */
    {0x11, 0x12, 0x14, 0x18, 0x14, 0x12, 0x11}, /* 'K' */
    {0x1E, 0x11, 0x11, 0x1E, 0x11, 0x11, 0x1E}, /* 'B' */
    {0x1F, 0x11, 0x11, 0x1F, 0x00, 0x00, 0x00}, /* 'P' */
};

static int
glyph_index(char c)
{
    if (c >= '0' && c <= '9') return c - '0';
    if (c == ':') return 10;
    if (c == 'K') return 11;
    if (c == 'B') return 12;
    if (c == 'P') return 13;
    return -1;
}

/* ------------------------------------------------------------- état client */

struct waytest {
    struct wl_display *dpy;
    struct wl_registry *reg;
    struct wl_compositor *comp;
    struct wl_shm *shm;
    struct wl_seat *seat;
    struct wl_pointer *ptr;
    struct wl_keyboard *kbd;
    struct xdg_wm_base *wmbase;
    struct xdg_surface *xsurf;
    struct xdg_toplevel *top;
    struct wl_surface *surf;

    int32_t w, h;               /* taille buffer actuel */
    int32_t pending_w, pending_h; /* taille demandée par le toplevel */
    struct wl_shm_pool *pool;
    struct wl_buffer *buf;
    uint32_t *pix;              /* mmap du pool */
    int shmfdfd;

    int tfd;                    /* timerfd horloge (1 s) */
    volatile sig_atomic_t quit;

    /* Compteurs affichés (preuves visuelles) et journalisés. */
    unsigned long kcount;       /* touches pressées */
    unsigned long pcount;       /* événements pointeur (motion/axis) */
    unsigned long bcount;       /* boutons pressés */

    /* Requêtes interactives xdg (TEST-GFX-001) : la 1re pression
     * interactive livrée (milieu ou droit) déclenche move, la 2e resize
     * — le TYPE est découplé du bouton physique, car le canal PS/2
     * sacrifie des événements de bouton à la traversée evdev/libinput
     * (SYN_DROPPED pendant les rafales). La sérial passée est celle de
     * CETTE pression — le grab implicite correspond. */
    int icount;                 /* pressions interactives livrées */

    /* Position pointeur courant : l'objet (carré fuchsia) se rend ici —
     * la preuve visuelle du « déplacement d'un objet au pointeur ». */
    int32_t lsq_x, lsq_y;       /* -1 : jamais entré */
};

static struct waytest S;

/* ---------------------------------------------------------------- rendu */

static void
px_fill(struct waytest *s, uint32_t color)
{
    for (int32_t i = 0; i < s->w * s->h; i++) s->pix[i] = color;
}

static void
px_rect(struct waytest *s, int32_t x, int32_t y, int32_t w, int32_t h,
        uint32_t color)
{
    for (int32_t j = 0; j < h; j++) {
        int32_t py = y + j;
        if (py < 0 || py >= s->h) continue;
        for (int32_t i = 0; i < w; i++) {
            int32_t px = x + i;
            if (px < 0 || px >= s->w) continue;
            s->pix[py * s->w + px] = color;
        }
    }
}

static void
px_glyph(struct waytest *s, const uint8_t *g, int32_t x, int32_t y,
         int32_t scale, uint32_t color)
{
    for (int r = 0; r < 7; r++)
        for (int c = 0; c < 5; c++)
            if (g[r] & (0x10 >> c))
                px_rect(s, x + c * scale, y + r * scale, scale, scale, color);
}

static void
px_text(struct waytest *s, const char *txt, int32_t x, int32_t y,
        int32_t scale, uint32_t color)
{
    /* largeur : 4 colonnes + 1 colonne d'espace par glyphe */
    for (const char *c = txt; *c; c++) {
        int gi = glyph_index(*c);
        if (gi >= 0) px_glyph(s, GLYPH[gi], x, y, scale, color);
        x += 6 * scale;
    }
}

static void
draw(struct waytest *s)
{
    if (!s->buf) {
        fprintf(stderr, "waytest: draw sans buffer (ignoré)\n");
        return;
    }

    char line1[16];             /* horloge */
    char line2[32];             /* compteurs */
    time_t now = time(NULL);
    struct tm tmv;
    localtime_r(&now, &tmv);
    snprintf(line1, sizeof(line1), "%02d:%02d:%02d",
             tmv.tm_hour, tmv.tm_min, tmv.tm_sec);
    snprintf(line2, sizeof(line2), "K%lu P%lu B%lu",
             s->kcount, s->pcount, s->bcount);

    uint32_t bg = 0xFF100F0C;       /* XRGB : fond sombre */
    uint32_t border = 0xFF6ADB2A;   /* bord vert franc */
    uint32_t bar = 0xFF1C1B15;
    uint32_t fg = 0xFFE8E8E8;

    px_fill(s, bg);
    /* cadre */
    px_rect(s, 0, 0, s->w, 6, border);
    px_rect(s, 0, s->h - 6, s->w, 6, border);
    px_rect(s, 0, 0, 6, s->h, border);
    px_rect(s, s->w - 6, 0, 6, s->h, border);
    /* barre de titre */
    px_rect(s, 6, 6, s->w - 12, 40, bar);

    int32_t s1 = s->w / 60; if (s1 < 3) s1 = 3; if (s1 > 7) s1 = 7;
    int32_t w1 = 8 * 6 * s1;
    px_text(s, line1, (s->w - w1) / 2, (int32_t)(s->h * 0.35), s1, fg);

    int32_t s2 = s->w / 110; if (s2 < 2) s2 = 2; if (s2 > 4) s2 = 4;
    int32_t w2 = (int32_t)strlen(line2) * 6 * s2;
    px_text(s, line2, (s->w - w2) / 2, (int32_t)(s->h * 0.62), s2, border);

    /* Objet déplacé par le pointeur (carré fuchsia, rendu à la dernière
     * position motion reçue) — il bouge à chaque motion livrée. */
    if (s->lsq_x >= 0) {
        int32_t sz = 14;
        px_rect(s, s->lsq_x - sz / 2, s->lsq_y - sz / 2, sz, sz, 0xFFFF20FF);
        px_rect(s, s->lsq_x - sz / 2 + 3, s->lsq_y - sz / 2 + 3,
                sz - 6, sz - 6, 0xFFE8E8E8);
    }

    wl_surface_attach(s->surf, s->buf, 0, 0);
    wl_surface_damage_buffer(s->surf, 0, 0, INT32_MAX, INT32_MAX);
    wl_surface_commit(s->surf);
    wl_display_flush(s->dpy);
}

/* ------------------------------------------------------- buffers wl_shm */

static void
buffer_release(void *data, struct wl_buffer *buf)
{
    (void)data; (void)buf;
}

static const struct wl_buffer_listener buffer_listener = { buffer_release };

/* Recrée pool + buffer pour la taille courante. Rend 0/≤0 en échec. */
static int
realloc_buffer(struct waytest *s, int32_t w, int32_t h)
{
    if (s->buf) { wl_buffer_destroy(s->buf); s->buf = NULL; }
    if (s->pool) { wl_shm_pool_destroy(s->pool); s->pool = NULL; }
    if (s->pix) { munmap(s->pix, (size_t)s->w * s->h * 4); s->pix = NULL; }
    if (s->shmfdfd >= 0) { close(s->shmfdfd); s->shmfdfd = -1; }

    s->w = w; s->h = h;
    if (w <= 0 || h <= 0) return -1;

    char tmpl[] = "/tmp/.waytest-XXXXXX";
    int fd = mkstemp(tmpl);
    if (fd < 0) {
        fprintf(stderr, "waytest: mkstemp échec: %s\n", strerror(errno));
        return -1;
    }
    unlink(tmpl);
    size_t bytes = (size_t)w * h * 4;
    if (ftruncate(fd, (off_t)bytes) < 0) { close(fd); return -1; }

    s->pix = mmap(NULL, bytes, PROT_READ | PROT_WRITE, MAP_SHARED, fd, 0);
    if (s->pix == MAP_FAILED) {
        s->pix = NULL;
        close(fd);
        return -1;
    }
    s->shmfdfd = fd;

    s->pool = wl_shm_create_pool(s->shm, fd, (int32_t)bytes);
    if (!s->pool) {
        fprintf(stderr, "waytest: wl_shm_create_pool échec\n");
        return -1;
    }
    s->buf = wl_shm_pool_create_buffer(s->pool, 0, w, h, w * 4,
                                       WL_SHM_FORMAT_XRGB8888);
    if (!s->buf) {
        fprintf(stderr, "waytest: create_buffer échec\n");
        return -1;
    }
    wl_buffer_add_listener(s->buf, &buffer_listener, NULL);
    fprintf(stderr, "waytest: buffer %dx%d prêt\n", w, h);
    return 0;
}

/* ------------------------------------------------------------ xdgshell */

static void
wm_base_ping(void *data, struct xdg_wm_base *wmb, uint32_t serial)
{
    (void)data;
    xdg_wm_base_pong(wmb, serial);
}

static const struct xdg_wm_base_listener wm_base_listener = {
    .ping = wm_base_ping,
};

static void
xdg_surface_configure(void *data, struct xdg_surface *xs, uint32_t serial)
{
    struct waytest *s = data;
    fprintf(stderr, "waytest: xdg_surface configure serial=%u "
            "(pending %dx%d, actuel %dx%d)\n", serial, s->pending_w,
            s->pending_h, s->w, s->h);
    xdg_surface_ack_configure(xs, serial);
    /* Séquence xdg : toplevel.configure (positions taille en attente) PUIS
     * xdg_surface.configure → ack → commit (l'attachement AVANT l'ack est
     * un comportement non conforme, le compositor l'ignore). */
    if (s->pending_w > 0 && s->pending_h > 0 &&
        (s->pending_w != s->w || s->pending_h != s->h)) {
        if (realloc_buffer(s, s->pending_w, s->pending_h) == 0) draw(s);
    } else if (!s->buf) {
        /* compositor sans taille explicite : repli raisonnable 800x600. */
        if (realloc_buffer(s, 800, 600) == 0) draw(s);
    }
}

static const struct xdg_surface_listener xdg_surface_listener = {
    .configure = xdg_surface_configure,
};

static void
toplevel_configure(void *data, struct xdg_toplevel *tl,
                   int32_t width, int32_t height, struct wl_array *states)
{
    struct waytest *s = data;
    (void)tl; (void)states;
    fprintf(stderr, "waytest: toplevel configure %dx%d\n", width, height);
    if (width > 0 && height > 0) {
        s->pending_w = width;
        s->pending_h = height;
    }
}

static void
toplevel_close(void *data, struct xdg_toplevel *tl)
{
    struct waytest *s = data;
    (void)tl;
    fprintf(stderr, "waytest: toplevel close\n");
    s->quit = 1;
}

/* Events (xdg-shell <= 1.49) sans effet ici mais sujets OBLIGATOIRES du
 * listener : sans handler, libwayland plante le dispatch (« listener
 * function for opcode N is NULL ») dès que le compositor les émet. */
static void
toplevel_configure_bounds(void *data, struct xdg_toplevel *tl, int32_t width,
                          int32_t height)
{
    (void)data; (void)tl; (void)width; (void)height;
}

static void
toplevel_wm_caps(void *data, struct xdg_toplevel *tl, struct wl_array *caps)
{
    (void)data; (void)tl; (void)caps;
}

static const struct xdg_toplevel_listener toplevel_listener = {
    .configure = toplevel_configure,
    .close = toplevel_close,
    .configure_bounds = toplevel_configure_bounds,
    .wm_capabilities = toplevel_wm_caps,
};

/* -------------------------------------------------------------- pointeur */

static void
pointer_enter(void *data, struct wl_pointer *wp, uint32_t serial,
              struct wl_surface *surf, wl_fixed_t x, wl_fixed_t y)
{
    struct waytest *s = data;
    (void)wp; (void)serial; (void)surf;
    fprintf(stderr, "waytest: pointer enter (%d,%d)\n",
            wl_fixed_to_int(x), wl_fixed_to_int(y));
    s->lsq_x = wl_fixed_to_int(x);
    s->lsq_y = wl_fixed_to_int(y);
    s->pcount++;
    draw(s);
}

static void
pointer_leave(void *data, struct wl_pointer *wp, uint32_t serial,
              struct wl_surface *surf)
{
    (void)data; (void)wp; (void)serial; (void)surf;
    fprintf(stderr, "waytest: pointer leave\n");
}

static void
pointer_motion(void *data, struct wl_pointer *wp, uint32_t time,
               wl_fixed_t x, wl_fixed_t y)
{
    struct waytest *s = data;
    (void)wp; (void)time;
    fprintf(stderr, "waytest: pointer motion (%d,%d)\n",
            wl_fixed_to_int(x), wl_fixed_to_int(y));
    s->lsq_x = wl_fixed_to_int(x);
    s->lsq_y = wl_fixed_to_int(y);
    s->pcount++;
    draw(s);    /* le carré suit le pointeur (déplacement d'objet) */
}

static void
pointer_button(void *data, struct wl_pointer *wp, uint32_t serial,
               uint32_t time, uint32_t button, uint32_t state)
{
    struct waytest *s = data;
    (void)wp; (void)time;
    fprintf(stderr, "waytest: pointer button %u %s (B=%lu)\n",
            button, state == WL_POINTER_BUTTON_STATE_PRESSED ? "press" : "release",
            s->bcount + (state == WL_POINTER_BUTTON_STATE_PRESSED ? 1 : 0));
    if (state == WL_POINTER_BUTTON_STATE_PRESSED) {
        s->bcount++;
        draw(s);
        /* Requêtes interactives xdg (TEST-GFX-001) : 1re livraison =
         * xdg_toplevel_move, 2e = xdg_toplevel_resize (bord droit + bas),
         * les suivantes sans effet. La sérial passée est celle de CETTE
         * pression — le grab implicite du compositor correspond
         * (pointer.has_grab côté serveur). */
        if (button == 274 || button == 273) {
            s->icount++;
            if (s->icount == 1) {
                xdg_toplevel_move(s->top, s->seat, serial);
                fprintf(stderr, "waytest: move request envoyé (serial=%u)\n",
                        serial);
            } else if (s->icount == 2) {
                xdg_toplevel_resize(s->top, s->seat, serial,
                                    XDG_TOPLEVEL_RESIZE_EDGE_RIGHT |
                                    XDG_TOPLEVEL_RESIZE_EDGE_BOTTOM);
                fprintf(stderr, "waytest: resize request envoyé (serial=%u)\n",
                        serial);
            }
        }
    }
}

static void
pointer_axis(void *data, struct wl_pointer *wp, uint32_t time,
             uint32_t axis, wl_fixed_t value)
{
    struct waytest *s = data;
    (void)wp; (void)time; (void)axis; (void)value;
    s->pcount++;
}

static void
pointer_axis_discrete(void *data, struct wl_pointer *wp, uint32_t axis,
                      int32_t discrete)
{
    (void)data; (void)wp; (void)axis; (void)discrete;
}

/* Stubs signature exacte pour axis_source(time u) et axis_stop(time u, axis u)
 * — un stub partagé avec « frame » ne compile pas. */
static void
pointer_axis_stub(void *data, struct wl_pointer *wp, uint32_t value)
{
    (void)data; (void)wp; (void)value;
}

static void
pointer_axis_stop(void *data, struct wl_pointer *wp, uint32_t time,
                  uint32_t axis)
{
    (void)data; (void)wp; (void)time; (void)axis;
}

static void
pointer_frame(void *data, struct wl_pointer *wp)   /* v5, obligatoire */
{
    struct waytest *s = data;
    (void)wp;
    s->pcount++;        /* la frame clôt une série d'events pointeur */
}

static const struct wl_pointer_listener pointer_listener = {
    .enter = pointer_enter,
    .leave = pointer_leave,
    .motion = pointer_motion,
    .button = pointer_button,
    .axis = pointer_axis,
    /* seat v5+ : frame/axis_source/axis_stop/axis_discrete obligatoires
     * (sans handler, libwayland plante le dispatch). */
    .frame = pointer_frame,
    .axis_source = pointer_axis_stub,
    .axis_stop = pointer_axis_stop,
    .axis_discrete = pointer_axis_discrete,
};

/* -------------------------------------------------------------- clavier */

static void
keyboard_keymap(void *data, struct wl_keyboard *wk, uint32_t format, int32_t fd,
                uint32_t size)
{
    (void)data; (void)wk; (void)format; (void)fd; (void)size;
}

static void
keyboard_enter(void *data, struct wl_keyboard *wk, uint32_t serial,
               struct wl_surface *surf, struct wl_array *keys)
{
    struct waytest *s = data;
    (void)wk; (void)serial; (void)surf; (void)keys;
    fprintf(stderr, "waytest: keyboard enter (focus obtenu)\n");
    draw(s);
}

static void
keyboard_leave(void *data, struct wl_keyboard *wk, uint32_t serial,
               struct wl_surface *surf)
{
    (void)data; (void)wk; (void)serial; (void)surf;
    fprintf(stderr, "waytest: keyboard leave\n");
}

static void
keyboard_key(void *data, struct wl_keyboard *wk, uint32_t serial, uint32_t time,
             uint32_t key, uint32_t state)
{
    struct waytest *s = data;
    (void)wk; (void)serial; (void)time;
    if (state == WL_KEYBOARD_KEY_STATE_PRESSED) {
        s->kcount++;
        fprintf(stderr, "waytest: KEY press code=%u (K=%lu)\n", key, s->kcount);
        draw(s);
    }
}

static void
keyboard_modifiers(void *data, struct wl_keyboard *wk, uint32_t serial,
                   uint32_t dep, uint32_t lat, uint32_t locked,
                   uint32_t group)
{
    (void)data; (void)wk; (void)serial; (void)dep; (void)lat;
    (void)locked; (void)group;
}

static void
keyboard_repeat_info(void *data, struct wl_keyboard *wk, int32_t rate,
                     int32_t delay)
{
    (void)data; (void)wk; (void)rate; (void)delay;
}

static const struct wl_keyboard_listener keyboard_listener = {
    .keymap = keyboard_keymap,
    .enter = keyboard_enter,
    .leave = keyboard_leave,
    .key = keyboard_key,
    .modifiers = keyboard_modifiers,
    .repeat_info = keyboard_repeat_info,
};

/* ----------------------------------------------------------------- seat */

static void
seat_capabilities(void *data, struct wl_seat *seat, uint32_t caps)
{
    struct waytest *s = data;
    if ((caps & WL_SEAT_CAPABILITY_POINTER) && !s->ptr) {
        s->ptr = wl_seat_get_pointer(seat);
        wl_pointer_add_listener(s->ptr, &pointer_listener, s);
        fprintf(stderr, "waytest: wl_pointer acquis\n");
    }
    if ((caps & WL_SEAT_CAPABILITY_KEYBOARD) && !s->kbd) {
        s->kbd = wl_seat_get_keyboard(seat);
        wl_keyboard_add_listener(s->kbd, &keyboard_listener, s);
        fprintf(stderr, "waytest: wl_keyboard acquis\n");
    }
}

static void
seat_name(void *data, struct wl_seat *seat, const char *name)
{
    (void)data; (void)seat; (void)name;
}

static const struct wl_seat_listener seat_listener = {
    .capabilities = seat_capabilities,
    .name = seat_name,
};

/* ------------------------------------------------------------- registry */

static void
registry_global(void *data, struct wl_registry *reg, uint32_t name,
                const char *iface, uint32_t version)
{
    struct waytest *s = data;
    if (strcmp(iface, "wl_compositor") == 0 && !s->comp) {
        s->comp = wl_registry_bind(reg, name, &wl_compositor_interface, 4);
    } else if (strcmp(iface, "wl_shm") == 0 && !s->shm) {
        s->shm = wl_registry_bind(reg, name, &wl_shm_interface, 1);
    } else if (strcmp(iface, "wl_seat") == 0 && !s->seat) {
        uint32_t v = version > 5 ? 5 : version;
        s->seat = wl_registry_bind(reg, name, &wl_seat_interface, v);
        wl_seat_add_listener(s->seat, &seat_listener, s);
        fprintf(stderr, "waytest: wl_seat v%u lié\n", v);
    } else if (strcmp(iface, "xdg_wm_base") == 0 && !s->wmbase) {
        s->wmbase = wl_registry_bind(reg, name, &xdg_wm_base_interface,
                                     version > 6 ? 6 : version);
        xdg_wm_base_add_listener(s->wmbase, &wm_base_listener, s);
        fprintf(stderr, "waytest: xdg_wm_base v%u lié\n", version);
    }
}

static void
registry_global_remove(void *data, struct wl_registry *reg, uint32_t name)
{
    (void)data; (void)reg; (void)name;
}

static const struct wl_registry_listener registry_listener = {
    .global = registry_global,
    .global_remove = registry_global_remove,
};

/* --------------------------------------------------------------- surface */

/* Events wl_surface (enter/leave, + scale/transform v3-4) : stubs requis
 * (dispatch NULL sinon). Le compteur P suit le pointeur, pas la surface. */
static void
surface_stub(void *data, struct wl_surface *ws, struct wl_output *o)
{
    (void)data; (void)ws; (void)o;
}

static void
surface_stub_int(void *data, struct wl_surface *ws, int32_t factor)
{
    (void)data; (void)ws; (void)factor;
}

static void
surface_stub_transform(void *data, struct wl_surface *ws, uint32_t transform)
{
    (void)data; (void)ws; (void)transform;
}

static const struct wl_surface_listener surface_listener = {
    .enter = surface_stub,
    .leave = surface_stub,
    .preferred_buffer_scale = surface_stub_int,
    .preferred_buffer_transform = surface_stub_transform,
};

/* ---------------------------------------------------------------- setup */

static void
on_term(int sig)
{
    (void)sig;
    S.quit = 1;
}

int
main(int argc, char **argv)
{
    (void)argc; (void)argv;
    setvbuf(stderr, NULL, _IOLBF, 0);
    memset(&S, 0, sizeof(S));
    S.shmfdfd = -1;

    S.dpy = wl_display_connect(NULL);
    if (!S.dpy) {
        fprintf(stderr, "waytest: échec connexion (WAYLAND_DISPLAY ?, "
                        "XDG_RUNTIME_DIR ?): %s\n", strerror(errno));
        return 1;
    }
    S.lsq_x = S.lsq_y = -1;
    S.reg = wl_display_get_registry(S.dpy);
    wl_registry_add_listener(S.reg, &registry_listener, &S);
    wl_display_roundtrip(S.dpy);
    if (!S.comp || !S.shm || !S.seat || !S.wmbase) {
        fprintf(stderr, "waytest: globals manquants (comp=%p shm=%p seat=%p "
                        "wm=%p)\n", (void *)S.comp, (void *)S.shm,
                (void *)S.seat, (void *)S.wmbase);
        return 1;
    }

    S.surf = wl_compositor_create_surface(S.comp);
    wl_surface_add_listener(S.surf, &surface_listener, &S);
    S.xsurf = xdg_wm_base_get_xdg_surface(S.wmbase, S.surf);
    xdg_surface_add_listener(S.xsurf, &xdg_surface_listener, &S);
    S.top = xdg_surface_get_toplevel(S.xsurf);
    xdg_toplevel_add_listener(S.top, &toplevel_listener, &S);
    xdg_toplevel_set_title(S.top, "waytest");
    /* Taille demandée raisonnable pour la QA : guider le compositor sans
     * imposer de min/max (pas de demande bloquante). */
    xdg_toplevel_set_min_size(S.top, 480, 320);
    wl_surface_commit(S.surf);
    wl_display_flush(S.dpy);

    S.tfd = timerfd_create(CLOCK_MONOTONIC, TFD_NONBLOCK | TFD_CLOEXEC);
    struct itimerspec its = { {1, 0}, {1, 0} };
    timerfd_settime(S.tfd, 0, &its, NULL);

    struct sigaction sa = { .sa_handler = on_term, .sa_flags = 0 };
    sigaction(SIGTERM, &sa, NULL);
    sigaction(SIGINT, &sa, NULL);

    /* Premier configure en attente : dessiner dès réception (la taille
     * exacte vient du compositor). Boucle : entrées + horloge 1 s. */
    struct pollfd pfd[2];
    pfd[0].fd = wl_display_get_fd(S.dpy);
    pfd[0].events = POLLIN;
    pfd[1].fd = S.tfd;
    pfd[1].events = POLLIN;

    fprintf(stderr, "waytest: prêt (boucle d'événements en cours)\n");
    while (!S.quit) {
        wl_display_flush(S.dpy);
        int r = poll(pfd, 2, 2000);
        if (r < 0) { if (errno == EINTR) continue; break; }
        if (pfd[0].revents & (POLLIN | POLLHUP | POLLERR)) {
            if (wl_display_dispatch(S.dpy) < 0) {
                fprintf(stderr, "waytest: erreur protocole, sortie\n");
                break;
            }
        }
        if (pfd[1].revents & POLLIN) {
            uint64_t ticks;
            ssize_t n = read(S.tfd, &ticks, sizeof(ticks));
            (void)n;
            fprintf(stderr, "waytest: seconde %lu (K=%lu P=%lu B=%lu)\n",
                    (unsigned long)ticks, S.kcount, S.pcount, S.bcount);
            draw(&S);
        }
    }
    fprintf(stderr, "waytest: fin propre (K=%lu P=%lu B=%lu)\n",
            S.kcount, S.pcount, S.bcount);
    return 0;
}
