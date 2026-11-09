#!/usr/bin/env python3
# SoleOS : images de marque (logo du démarrage, pastille de version, fonds du menu de démarrage)
# marque.py <dossier de sortie> <version, ex. v12.1>
import sys
from PIL import Image, ImageDraw, ImageFont, ImageFilter
import os
F=os.path.join(os.path.dirname(os.path.abspath(__file__)),'..','polices','SpaceGrotesk.ttf')
J=(240,160,70); S=4
LOGO_S=os.path.join(os.path.dirname(os.path.abspath(__file__)),'logo-s.png')
sortie, version = sys.argv[1], sys.argv[2]
def police(t,w=700):
    f=ImageFont.truetype(F,t); f.set_variation_by_axes([w]); return f
def eclair(taille,ep=2.0,couleur=J):
    """Logo « S » de SoleOS (nom gardé pour la suite du script)."""
    return Image.open(LOGO_S).convert('RGBA').resize((taille,taille),Image.LANCZOS)
def logo(W,H,e,t):
    im=Image.new('RGBA',(W,H),(0,0,0,0))
    f=police(t); d=ImageDraw.Draw(im); tw=d.textlength('SoleOS',font=f)
    gap=int(e*0.26); tot=e+gap+tw; x0=(W-tot)/2; cy=H/2
    g=Image.new('RGBA',(W,H),(0,0,0,0)); r=e*0.6
    ImageDraw.Draw(g).ellipse((x0+e/2-r,cy-r,x0+e/2+r,cy+r),fill=(80,130,240,60))
    g=g.filter(ImageFilter.GaussianBlur(e*0.3)); im=Image.alpha_composite(im,g)
    im.alpha_composite(eclair(e),(int(x0),int(cy-e/2)))
    d=ImageDraw.Draw(im); bb=d.textbbox((0,0),'SoleOS',font=f)
    ws=d.textlength('Sole',font=f)
    d.text((x0+e+gap,cy-(bb[1]+bb[3])/2),'Sole',font=f,fill=(242,244,248))
    d.text((x0+e+gap+ws,cy-(bb[1]+bb[3])/2),'OS',font=f,fill=J)
    return im
def pastille(v,t=26):
    f=police(t,500); tw=ImageDraw.Draw(Image.new('RGBA',(1,1))).textlength(v,font=f); w=int(tw+46); h=46
    im=Image.new('RGBA',(w*S,h*S),(0,0,0,0)); d=ImageDraw.Draw(im)
    d.rounded_rectangle((S,S,w*S-S,h*S-S),radius=h*S/2-S,outline=J+(235,),width=2*S,fill=J+(30,))
    im=im.resize((w,h),Image.LANCZOS); d=ImageDraw.Draw(im); bb=d.textbbox((0,0),v,font=f)
    d.text(((w-tw)/2,(h-(bb[1]+bb[3]))/2),v,font=f,fill=J); return im
def splash(W,H,e,t):
    base=Image.new('RGB',(W,H),(14,19,36)); g=Image.new('L',(W,H),0)
    ImageDraw.Draw(g).ellipse((W*0.15,H*0.05,W*0.85,H*0.95),fill=255); g=g.filter(ImageFilter.GaussianBlur(W/8))
    im=Image.composite(Image.new('RGB',(W,H),(22,30,58)),base,g).convert('RGBA')
    lw,lh=int(W*0.45),int(e*2.2); im.alpha_composite(logo(lw,lh,e,t),((W-lw)//2,(H-lh)//2)); return im.convert('RGB')
logo(640,220,112,92).save(f'{sortie}/logo.png')
pastille(version).save(f'{sortie}/version.png')
splash(1920,1080,104,84).save(f'{sortie}/splash-1920x1080.png')
splash(640,480,52,42).save(f'{sortie}/splash-640x480.png')
