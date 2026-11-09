#!/bin/bash
# Recopie les fichiers de flashos-installateur/systeme dans construire-iso-flashos.sh (entre les repères)
set -e
cd /home/claude
S=flashos-installateur/systeme
B=construire-iso-flashos.sh
bloc=$(mktemp)
{
  echo '# >>> FICHIERS SYSTEME (générés depuis flashos-installateur/systeme : ne pas modifier ici)'
  (cd $S && find . -type f | sed 's#^\./##' | sort) | while read -r f; do
    echo "mkdir -p \"\$I/$(dirname "$f")\""
    echo "cat > \"\$I/$f\" <<'FINSYS'"
    cat "$S/$f"
    echo "FINSYS"
  done
  (cd $S && find . -type f | sed 's#^\./##' | sort) | while read -r f; do
    case "$f" in etc/sudoers.d/*) echo "chmod 440 \"\$I/$f\"" ;; *) if [ -x "$S/$f" ]; then echo "chmod 755 \"\$I/$f\""; fi ;; esac
  done
  echo '# <<< FICHIERS SYSTEME'
} > $bloc
python3 - "$B" "$bloc" <<'PY'
import sys
b=open(sys.argv[1]).read(); bl=open(sys.argv[2]).read()
d=b.index('# >>> FICHIERS SYSTEME'); f=b.index('# <<< FICHIERS SYSTEME')+len('# <<< FICHIERS SYSTEME\n')
open(sys.argv[1],'w').write(b[:d]+bl+b[f:])
PY
rm -f $bloc
bash -n $B && echo "construire-iso-flashos.sh : bloc système régénéré ($(ls -R $S | wc -l))"
