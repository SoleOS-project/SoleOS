import re
def chaines(s):
    """Renvoie [(debut, fin, contenu)] des littéraux "..." (hors bruts, commentaires, caractères)."""
    out=[]; i=0; n=len(s)
    while i<n:
        c=s[i]
        if s.startswith('//',i):
            j=s.find('\n',i); i=n if j<0 else j; continue
        if s.startswith('/*',i):
            j=s.find('*/',i); i=n if j<0 else j+2; continue
        m=re.match(r'b?r(#*)"',s[i:])
        if m and (i==0 or not (s[i-1].isalnum() or s[i-1]=='_')):
            fin='"'+m.group(1); j=s.find(fin,i+len(m.group(0))); i=j+len(fin); continue
        if c=="'":
            m=re.match(r"'(\\u\{[0-9a-fA-F]+\}|\\.|[^\\'])'",s[i:])
            if m: i+=len(m.group(0)); continue
            i+=1; continue
        if c=='"':
            j=i+1
            while j<n and s[j]!='"':
                j+= 2 if s[j]=='\\' else 1
            out.append((i,j+1,s[i+1:j])); i=j+1; continue
        i+=1
    return out

# fonction -> positions des arguments qui sont des textes à traduire
MOTS = {'tr': (0,), 'trv': (0,), 'n_': (0,), 'selon': (0, 1), 'etiquette': (0,), 'petit': (0,),
        'petit_bouton': (0,), 'titre_section': (0,), 'saisie_secrete': (0,), 'bouton': (0,),
        'choix_fond': (1,), 'bouton_etat': (1,), 'entree_menu': (0,), 'carte_tete': (1,), 'ligne_reglage': (1,), 'ligne': (1, 2), 'page_travail': (1, 2), 'suivre': (1,)}

_CACHE = {}
def _fins(s):
    """Fin (exclue) -> début de chaque littéral, pour les sauter en remontant le texte."""
    k = id(s)
    if k not in _CACHE or _CACHE[k][0] is not s:
        _CACHE[k] = (s, {b: a for a, b, _ in chaines(s)})
    return _CACHE[k][1]

def appel(s, debut):
    """Fonction et position d'argument du littéral commençant à `debut` (ou None)."""
    fins = _fins(s)
    prof = 0; virgules = 0; i = debut - 1
    while i >= 0:
        if i + 1 in fins:
            i = fins[i + 1] - 1; continue
        c = s[i]
        if c in ')]}': prof += 1
        elif c in '([{':
            if prof == 0:
                if c != '(': return None
                m = re.search(r'([A-Za-z_][A-Za-z0-9_]*)\s*!?\s*$', s[:i])
                return (m.group(1), virgules) if m else None
            prof -= 1
        elif c == ',' and prof == 0: virgules += 1
        elif c == ';' and prof == 0: return None
        i -= 1
    return None


def extrait(s, a):
    """Vrai si le littéral commençant en `a` est un texte à traduire (argument d'une fonction de MOTS)."""
    r = appel(s, a)
    if not r: return False
    nom, pos = r
    return nom in MOTS and pos in MOTS[nom] and re.search(r'[(,]\s*&?\s*$', s[:a]) is not None
