"""Generate a low-poly red/gold armor prototype GLB and texture atlas.

Requires Python packages: trimesh, numpy, Pillow.
This is a preview asset only, not a GTA V ped or a Glue5 runtime model.
"""
import os
import numpy as np
import trimesh
from PIL import Image, ImageDraw

OUT = os.path.dirname(os.path.abspath(__file__))
SIZE = 1024
image = Image.new("RGB", (SIZE, SIZE), (18, 22, 30))
draw = ImageDraw.Draw(image)
red, red_hi = (154, 22, 31), (212, 42, 45)
gold, gold_hi = (231, 169, 53), (255, 211, 100)
dark, cyan, edge = (28, 35, 45), (55, 221, 255), (70, 79, 91)

def panel(box, fill):
    draw.rounded_rectangle(box, radius=28, fill=fill, outline=edge, width=8)

panel((40, 40, 480, 500), red)
draw.polygon([(110,100),(255,75),(410,100),(385,240),(255,320),(135,240)],
             fill=red_hi, outline=gold_hi)
draw.ellipse((198,145,312,259), fill=gold, outline=gold_hi, width=10)
draw.ellipse((218,165,292,239), fill=cyan, outline=(210,250,255), width=8)
panel((530,40,980,350), gold)
draw.polygon([(610,95),(900,95),(870,220),(760,290),(650,220)],
             fill=red, outline=gold_hi)
draw.polygon([(655,150),(745,155),(730,183),(650,183)], fill=cyan)
draw.polygon([(775,155),(865,150),(870,183),(790,183)], fill=cyan)
draw.line((760,190,760,260), fill=gold_hi, width=12)
for i, box in enumerate([(40,550,245,760),(275,550,480,760),
                         (530,400,735,610),(765,400,970,610),
                         (530,650,735,860),(765,650,970,860)]):
    fill = red if i % 2 == 0 else gold
    panel(box, fill)
    x1,y1,x2,y2 = box
    draw.line((x1+30,y1+40,x2-30,y1+40),
              fill=gold_hi if fill == red else red_hi, width=12)
    draw.line((x1+35,y2-45,x2-35,y2-45), fill=edge, width=8)
for x,y in [(85,800),(315,800),(570,900),(800,900)]:
    draw.rounded_rectangle((x,y,x+130,y+45), radius=10, fill=dark, outline=gold, width=4)
    draw.ellipse((x+50,y+8,x+80,y+38), fill=cyan)
image.save(os.path.join(OUT, "iron-man-armor-atlas.png"))

scene = trimesh.Scene()
def box(name, center, size, color, textured=False):
    mesh = trimesh.creation.box(extents=size)
    mesh.apply_translation(center)
    if textured:
        v = mesh.vertices
        uv = np.column_stack((0.04 + (v[:,0]+size[0]/2)/size[0]*0.43,
                              0.52 + (v[:,2]+size[2]/2)/size[2]*0.44))
        material = trimesh.visual.material.PBRMaterial(
            baseColorTexture=image, metallicFactor=0.65, roughnessFactor=0.3)
        mesh.visual = trimesh.visual.texture.TextureVisuals(uv=uv, material=material)
    else:
        mesh.visual.vertex_colors = np.tile(np.array([*color,255], dtype=np.uint8),
                                            (len(mesh.vertices),1))
    scene.add_geometry(mesh, node_name=name, geom_name=name)

def sphere(name, center, radius, color, scale=(1,1,1), subdivisions=2):
    mesh = trimesh.creation.icosphere(subdivisions=subdivisions, radius=radius)
    mesh.apply_scale(scale)
    mesh.apply_translation(center)
    mesh.visual.vertex_colors = np.tile(np.array([*color,255], dtype=np.uint8),
                                        (len(mesh.vertices),1))
    scene.add_geometry(mesh, node_name=name, geom_name=name)

box("Chest_Armor",(0,0,1.38),(0.72,0.38,0.68),red,True)
box("Abdomen",(0,0,0.98),(0.48,0.34,0.22),dark)
box("Pelvis",(0,0,0.78),(0.52,0.36,0.25),gold)
box("Neck",(0,0,1.79),(0.20,0.20,0.14),dark)
sphere("Helmet",(0,0,1.99),0.25,gold,(0.86,0.8,1.05),3)
box("Faceplate",(0,-0.195,1.99),(0.30,0.055,0.24),red)
box("Left_Eye",(-0.085,-0.229,2.025),(0.09,0.02,0.025),cyan)
box("Right_Eye",(0.085,-0.229,2.025),(0.09,0.02,0.025),cyan)
for side, label in [(-1,"L"),(1,"R")]:
    sphere(label+"_Shoulder",(side*0.48,0,1.58),0.19,gold,(1,1,0.9))
    box(label+"_UpperArm",(side*0.55,0,1.34),(0.22,0.25,0.42),red)
    box(label+"_Forearm",(side*0.58,-0.015,1.02),(0.24,0.28,0.36),gold)
    sphere(label+"_Hand",(side*0.58,-0.015,0.80),0.12,dark,(0.9,0.75,1.0))
    box(label+"_Thigh",(side*0.17,0,0.54),(0.24,0.30,0.40),red)
    box(label+"_Knee",(side*0.17,-0.01,0.32),(0.23,0.31,0.14),gold)
    box(label+"_Shin",(side*0.17,0,0.13),(0.22,0.29,0.28),red)
    box(label+"_Boot",(side*0.17,-0.07,-0.02),(0.26,0.40,0.12),dark)
scene.export(os.path.join(OUT, "iron-man-armor-prototype.glb"))
print("Created GLB prototype and PNG atlas in", OUT)
