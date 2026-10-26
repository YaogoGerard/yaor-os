/*
 * xwtest — client X11 de validation (ADR-024, TEST-GFX-003, v0.4 étape 4).
 *
 * Fenêtre X11 (Xlib) servie par Xwayland via le compositor YAOR : fond bleu
 * franc (0x2A6ADB — distinct des fenêtres waytest vert/orange), carré blanc
 * central dessiné à la première Expose pour la preuve pixel côté hôte
 * (screendump). Journalage stderr de chaque événement reçu :
 *
 *   « xwtest: MAP id=0x… »        fenêtre mappée côté X
 *   « xwtest: BTN n=1 »           clic livré par la session (pointer)
 *   « xwtest: KEY keysym=… »      touche livrée par la session (clavier)
 *   « xwtest: UNMAP »             démappage (quittement par le compositeur)
 *
 * Les jetons courts passent les troncatures d'écho de la console série :
 * le harnais QA grep côté invité (dmesg/journal) plutôt que l'echo hôte.
 *
 * Dépendances runtime : libX11 seule (fond du protocole : XCB en interne).
 * Aucun texte via polices cœur (pas de freetype au live) : la géométrie
 * suffît à la preuve. Sources internes (« un besoin = un fournisseur »).
 *
 * Sortie : touche 'q' (toujours journalisée) quitte ; le reste est un
 * évènementiel passif. journal : stderr.
 */

#include <stdio.h>
#include <stdlib.h>
#include <X11/Xlib.h>
#include <X11/Xutil.h>
#include <X11/keysym.h>

/* Couleurs de la fenêtre de preuve (déclinées de waytest pour le
 * contraste visuel dans le screendump : W1 vert, W2 orange, X11 bleu). */
#define BG_BLUE   0x2A6ADB
#define BORDER    0x0B1B3A
#define MARKER    0xFFFFFF

int main(void) {
    setvbuf(stdout, NULL, _IONBF, 0);
    setvbuf(stderr, NULL, _IONBF, 0);

    if (!getenv("DISPLAY") || !*getenv("DISPLAY")) {
        fprintf(stderr, "xwtest: DISPLAY vide\n");
        return 1;
    }
    Display *d = XOpenDisplay(NULL);
    if (!d) {
        fprintf(stderr, "xwtest: connexion X impossible\n");
        return 1;
    }
    int scr = DefaultScreen(d);
    Window w = XCreateSimpleWindow(d, RootWindow(d, scr), 40, 40, 480, 360, 4,
                                   BORDER, BG_BLUE);
    if (!w) {
        fprintf(stderr, "xwtest: fenêtre refusée\n");
        return 1;
    }
    XSelectInput(d, w, ExposureMask | KeyPressMask | ButtonPressMask |
                       StructureNotifyMask);

    XMapWindow(d, w);
    fprintf(stderr, "xwtest: cmd OK id=0x%lx\n", (unsigned long)w);

    GC gc = DefaultGC(d, scr);
    for (;;) {
        XEvent ev;
        XNextEvent(d, &ev);
        switch (ev.type) {
            case Expose: {
                XSetForeground(d, gc, BG_BLUE);
                XFillRectangle(d, w, gc, 0, 0, 480, 360);
                XSetForeground(d, gc, MARKER);
                /* carré blanc central : preuve pixel unique */
                XFillRectangle(d, w, gc, 190, 130, 100, 100);
                XFlush(d);
                break;
            }
            case MapNotify:
                fprintf(stderr, "xwtest: MAP id=0x%lx\n",
                        (unsigned long)ev.xmap.window);
                break;
            case UnmapNotify:
                fprintf(stderr, "xwtest: UNMAP\n");
                break;
            case ButtonPress:
                fprintf(stderr, "xwtest: BTN n=%u\n", ev.xbutton.button);
                break;
            case KeyPress: {
                KeySym ks = 0;
                char buf[16];
                int n = XLookupString(&ev.xkey, buf, (int)sizeof(buf) - 1, &ks, NULL);
                buf[n < 0 ? 0 : (n < (int)sizeof(buf) ? n : (int)sizeof(buf) - 1)] = '\0';
                fprintf(stderr, "xwtest: KEY keysym=0x%lx char=%s\n",
                        (unsigned long)ks, n > 0 ? buf : "");
                if (ks == XK_q) {
                    XDestroyWindow(d, w);
                    XCloseDisplay(d);
                    return 0;
                }
                break;
            }
            case DestroyNotify:
                XCloseDisplay(d);
                return 0;
            default:
                break;
        }
    }
}
