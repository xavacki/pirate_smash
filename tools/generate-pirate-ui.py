from pathlib import Path
from PIL import Image,ImageDraw,ImageFont
import shutil,json,math
R=Path(__file__).resolve().parents[1];O=R/'assets/pirate'
# Extend existing atlas, preserving all original sprite coordinates and startup branding.
a=Image.new('RGBA',(4096,4096));old=Image.open(R/'assets/start-screen/atlas.png');a.paste(old.crop((0,0,4096,2278)),(0,0));a.putpixel((0,0),(255,255,255,255))
wood=Image.open(O/'wood.png').convert('RGBA')
def panel(size,button=False):
    w,h=size; im=Image.new('RGBA',size);d=ImageDraw.Draw(im)
    for y in range(h):
        if button:c=(5+int(y/h*6),45+int(y/h*42),60+int(y/h*50),255)
        else:c=(49+int(y/h*8),25+int(y/h*5),16,255)
        d.line((0,y,w,y),fill=c)
    if not button:
        grain=wood.resize(size);grain.putalpha(85);im=Image.alpha_composite(im,grain);d=ImageDraw.Draw(im)
        for y in range(40,h,50):d.line((8,y,w-8,y),fill=(24,13,9),width=3);d.line((8,y+3,w-8,y+3),fill=(122,70,30),width=1)
    pts=[(0,16),(16,0),(w-17,0),(w-1,16),(w-1,h-17),(w-17,h-1),(16,h-1),(0,h-17),(0,16)]
    d.line(pts,fill=(49,24,12),width=16);d.line(pts,fill=(182,110,32),width=10);d.line(pts,fill=(255,213,121),width=3)
    for x in (16,w-17):
        for y in (16,h-17):d.ellipse((x-5,y-5,x+5,y+5),fill=(230,170,64),outline=(59,32,14),width=2);d.line((x-2,y-2,x+1,y+1),fill=(255,235,170),width=2)
    return im
p=panel((1024,240));a.paste(p,(0,2304));a.paste(panel((768,140),True),(1100,2304))
# Emblem is a separate native atlas element, with gold rim and skull/sabres.
im=Image.new('RGBA',(256,256));d=ImageDraw.Draw(im)
d.ellipse((28,20,228,224),fill=(31,23,21),outline=(230,166,56),width=10)
for points in [[(35,205),(199,50)],[(221,205),(57,50)]]:
    d.line(points,fill=(55,32,18),width=22);d.line(points,fill=(216,209,185),width=12)
d.ellipse((85,65,171,149),fill=(245,211,140),outline=(126,79,28),width=4)
d.rounded_rectangle((103,124,154,166),radius=5,fill=(245,211,140))
for x in (99,135):d.ellipse((x,98,x+24,121),fill=(25,18,17))
d.polygon([(125,119),(116,136),(136,136)],fill=(51,27,15))
for x in range(111,151,9):d.line((x,143,x,165),fill=(83,48,23),width=3)
d.polygon([(58,84),(78,43),(104,56),(128,30),(152,56),(178,43),(198,84)],fill=(15,24,29),outline=(239,171,59),width=4)
d.arc((60,59,198,100),0,180,fill=(197,35,23),width=10)
a.paste(im,(1920,2304))
# A serif glyph atlas gives the HUD the reference's engraved nautical lettering.
f=ImageFont.truetype('/usr/share/fonts/truetype/dejavu/DejaVuSerif-Bold.ttf',52)
for idx,code in enumerate(range(32,127)):
    ch=chr(code);tile=Image.new('RGBA',(48,64));dd=ImageDraw.Draw(tile)
    b=dd.textbbox((0,0),ch,font=f);mask=Image.new('RGBA',(max(1,b[2]-b[0]),max(1,b[3]-b[1])));ImageDraw.Draw(mask).text((-b[0],-b[1]),ch,font=f,fill='white')
    if code!=32:
        mask.thumbnail((44,54));tile.paste(mask,((48-mask.width)//2,(64-mask.height)//2))
    a.paste(tile,((idx%32)*48,2688+(idx//32)*64))
# Harbor image in atlas is for the home menu only; gameplay uses a scene background.
bg=Image.open(O/'harbor.png').convert('RGBA');bg.thumbnail((900,1152));a.paste(bg,(2304,2304))
(O/'ui-regions.json').write_text(json.dumps({'panel':[0,2304,1024,240],'button':[1100,2304,768,140],'crest':[1920,2304,256,256],'home':[2304,2304,bg.width,bg.height]},indent=2))
a.save(R/'assets/start-screen/atlas.png')
p=R/'assets/start-screen/atlas_map.rs';s=p.read_text().replace('ATLAS_HEIGHT:u32=2278','ATLAS_HEIGHT:u32=4096');p.write_text(s)
# Store coordinates as generated Rust constants.
(R/'assets/pirate/ui_map.rs').write_text(f'pub const HOME: [f32; 4] = [2304., 2304., {bg.width}., {bg.height}.];\n')
print('Pirate HUD atlas and harbor installed')
