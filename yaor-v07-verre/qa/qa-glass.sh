#!/bin/sh
# QA v0.7 « Verre » — à lancer DANS l'invité (session desktop démarrée).
# Chaque contrôle imprime PASS/FAIL ; sortie 0 si tout passe.
# Les jetons attendus sont CALCULÉS à l'exécution (règle QA du brief : jamais
# un motif littéral dans l'écho tty).
#
# Environnement rigoureux (constats v0.7 QEMU) :
# - profil classic => PAS de dock ; on le lance ici (comme en session macos)
#   AVANT les bascules, pour que ses jetons existent dans le journal.
# - « fond flouté » est logé par le compositor SEULEMENT quand une fenêtre
#   xdg-shell est mappée ; on en lance une (yaor-settings) en fond.
# - motifs sans accents dans les greps : l'écho tty casse les caractères
#   accentués tapés depuis la console série (« flout » couvre « flouté »).
# - jetons clients : tail -F du fd après lancement ne suffit pas, on inter.
#   roge journalctl avec motif ASCII 'verre : ' + compteur depuis le début.
fail=0
ok()  { echo "PASS $1"; }
ko()  { echo "FAIL $1"; fail=1; }
chk() { if [ "$2" = "$3" ]; then ok "$1"; else ko "$1 (attendu '$3', lu '$2')"; fi; }

# dock + fenêtre de test (la session classic ne met ni l'un ni l'autre)
# l'unité est « active » dès l'exec : attendre le socket wayland réel
# (résolution du nom DANS la boucle : avant l'exec du compositor, ls est vide)
W=""
i=0
until { [ -n "$W" ] && [ -S "/run/user/0/$W" ]; } || [ $i -ge 30 ]; do
    W=$(ls /run/user/0 | grep -x "wayland-[0-9]*" | head -1)
    i=$((i+1)); sleep 1
done
XDG_RUNTIME_DIR=/run/user/0 WAYLAND_DISPLAY="$W" nohup yaor-dock \
    >/tmp/yd.log 2>&1 & DCK=$!
XDG_RUNTIME_DIR=/run/user/0 WAYLAND_DISPLAY="$W" nohup yaor-settings \
    >/tmp/ys.log 2>&1 & SET=$!
sleep 4

J=/dev/shm/qa-glass-journal.txt
journalctl -b --no-pager > "$J" 2>/dev/null || journalctl --no-pager > "$J"

for p in glass-ambre glass-glacier glass-amethyste glass-emeraude flat; do
  yaor-ctl theme set "$p" 2>/tmp/ctl.err; rc=$?
  chk "set $p : code de sortie" "$rc" "0"
  chk "set $p : nom relu" "$(yaor-ctl theme get name)" "$p"
  want=glass; [ "$p" = flat ] && want=flat
  chk "set $p : style relu" "$(yaor-ctl theme get style)" "$want"
  sleep 2   # laisser le compositor relayer SIGHUP et les clients reconstruire
  kill -HUP $DCK 2>/dev/null   # dock externe : pas enfant du compositor ici
  journalctl -b --no-pager >> "$J" 2>/dev/null
  tok="verre : $( [ $want = glass ] && echo actif || echo plat )"
  n=$(grep -ac "yaor-panel.*$tok" "$J")
  [ "$n" -ge 1 ] && ok "$p : panel '$tok'" || ko "$p : panel '$tok' absent du journal"
  # le dock lancé ici (nohup) logue dans son fichier, pas au journal
  n=$(grep -ac "yaor-dock.*$tok" "$J" /tmp/yd.log | cut -d: -f2 | paste -sd+ | bc)
  [ "$n" -ge 1 ] && ok "$p : dock '$tok'" || ko "$p : dock '$tok' absent (journal+yd.log)"
done

# réglage unitaire + bornes + refus
yaor-ctl theme set glass-ambre 2>/dev/null
yaor-ctl theme tune glass_blur=999 2>/dev/null
chk "tune borne le flou à 48" "$(yaor-ctl theme get glass_blur)" "48"
chk "tune passe le nom à custom" "$(yaor-ctl theme get name)" "custom"
yaor-ctl theme tune accent=rouge 2>/dev/null; chk "couleur invalide refusée (code 2)" "$?" "2"
yaor-ctl theme set inconnu 2>/dev/null;       chk "préset inconnu refusé (code 2)" "$?" "2"

# retour au plat : doit redonner le rendu v0.6 (sondes pixel historiques valides)
yaor-ctl theme set flat 2>/dev/null
chk "retour au plat" "$(yaor-ctl theme get style)" "flat"

# compositor : le fond flouté est logé une fois par thème si une fenêtre
# xdg-shell est mappée (motif sans accent pour la console série)
journalctl -b --no-pager >> "$J" 2>/dev/null
n=$(grep -ac "fond flout" "$J")
[ "$n" -ge 1 ] && ok "compositor : fond flouté ($n lignes)" || ko "compositor : aucun 'fond flout*'"

kill $DCK $SET 2>/dev/null
[ $fail -eq 0 ] && echo "QA-GLASS: TOUT PASSE" || echo "QA-GLASS: ECHECS"
exit $fail
