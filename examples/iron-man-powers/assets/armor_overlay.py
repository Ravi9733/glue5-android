"""
Experimental Iron Man armor overlay generator for a rigged GTA V ped FBX.

Run with Blender:
  blender -b rigged_ped_template.fbx --python armor_overlay.py -- --out iron_man_overlay.fbx

This is a Blender-side preparation step, NOT a GTA V-ready .ydd/.yft pack.
Use a legally obtained/exported rigged ped template and keep a backup.
Armor plates are rigidly bone-parented; deformation, collision, GTA material
conversion and in-game loading still need to be validated in Sollumz/Glue5.
"""
import bpy
import sys
import math
from mathutils import Vector

ARGS = sys.argv[sys.argv.index("--") + 1:] if "--" in sys.argv else []
OUT = "iron_man_overlay.fbx"
if "--out" in ARGS:
    OUT = ARGS[ARGS.index("--out") + 1]

# Known GTA V skeleton bone-name variants. The script deliberately refuses
# to fabricate a skeleton when the supplied template does not contain one.
ALIASES = {
    "pelvis": ["SKEL_Pelvis", "Pelvis"],
    "spine": ["SKEL_Spine2", "SKEL_Spine1", "SKEL_Spine_Root", "Spine2", "Spine"],
    "head": ["SKEL_Head", "Head"],
    "l_upperarm": ["SKEL_L_UpperArm", "L_UpperArm"],
    "r_upperarm": ["SKEL_R_UpperArm", "R_UpperArm"],
    "l_forearm": ["SKEL_L_Forearm", "SKEL_L_ForeArm", "L_Forearm"],
    "r_forearm": ["SKEL_R_Forearm", "SKEL_R_ForeArm", "R_Forearm"],
    "l_thigh": ["SKEL_L_Thigh", "L_Thigh"],
    "r_thigh": ["SKEL_R_Thigh", "R_Thigh"],
    "l_calf": ["SKEL_L_Calf", "L_Calf"],
    "r_calf": ["SKEL_R_Calf", "R_Calf"],
}

def find_armature():
    armatures = [o for o in bpy.context.scene.objects if o.type == "ARMATURE"]
    if not armatures:
        raise RuntimeError("No armature found. Import a rigged ped FBX first.")
    # Prefer an armature containing recognizable GTA-style bones.
    for arm in armatures:
        names = {b.name for b in arm.data.bones}
        if any(n.startswith("SKEL_") for n in names):
            return arm
    return armatures[0]

arm = find_armature()
bone_names = {b.name for b in arm.data.bones}

def find_bone(key):
    for candidate in ALIASES[key]:
        if candidate in bone_names:
            return candidate
    return None

# Materials: red enamel and gold alloy. These are Blender preview materials;
# Sollumz shader conversion is a separate step.
def make_material(name, color, metallic=0.65, roughness=0.28):
    mat = bpy.data.materials.new(name)
    mat.diffuse_color = (*color, 1.0)
    mat.use_nodes = True
    bsdf = mat.node_tree.nodes.get("Principled BSDF")
    if bsdf:
        bsdf.inputs["Base Color"].default_value = (*color, 1.0)
        bsdf.inputs["Metallic"].default_value = metallic
        bsdf.inputs["Roughness"].default_value = roughness
    return mat

RED = make_material("IM_Armor_Red", (0.48, 0.018, 0.028))
GOLD = make_material("IM_Armor_Gold", (0.92, 0.52, 0.07))
DARK = make_material("IM_Armor_Joints", (0.035, 0.045, 0.06), 0.35, 0.42)

# Create a bevelled box and parent it to a named bone while preserving its
# world transform. Dimensions are intentionally conservative for a first pass.
def plate(name, location, dimensions, material, bone_key, bevel=0.06):
    bone_name = find_bone(bone_key)
    if not bone_name:
        print("SKIP", name, "- missing bone alias for", bone_key)
        return None
    bpy.ops.mesh.primitive_cube_add(size=1, location=location)
    obj = bpy.context.object
    obj.name = name
    obj.dimensions = dimensions
    bpy.ops.object.transform_apply(location=False, rotation=False, scale=True)
    obj.data.materials.append(material)
    if bevel > 0:
        mod = obj.modifiers.new("Armor edge bevel", "BEVEL")
        mod.width = bevel
        mod.segments = 2
        obj.modifiers.new("Weighted corner normals", "WEIGHTED_NORMAL")
    # Bone-parent the rigid plate so it follows that part of the template rig.
    world = obj.matrix_world.copy()
    obj.parent = arm
    obj.parent_type = "BONE"
    obj.parent_bone = bone_name
    obj.matrix_world = world
    return obj

def bone_mid(key):
    name = find_bone(key)
    if not name:
        return None
    b = arm.data.bones[name]
    # Convert rest-pose midpoint to world space.
    p = arm.matrix_world @ ((b.head_local + b.tail_local) * 0.5)
    return p

def at(key, offset=(0.0, 0.0, 0.0)):
    p = bone_mid(key)
    return p + Vector(offset) if p else None

# Torso plates centered around spine/pelvis; limb plates follow upper-arm,
# forearm, thigh and calf bones. Position offsets may need tuning per template.
parts = [
    ("IM_Chest_Core", "spine", (0, 0, 0.10), (0.42, 0.24, 0.42), RED),
    ("IM_Abdomen", "pelvis", (0, 0, 0.02), (0.34, 0.22, 0.28), GOLD),
    ("IM_Helmet", "head", (0, 0, 0.02), (0.25, 0.24, 0.30), RED),
    ("IM_L_Shoulder", "l_upperarm", (0, 0, 0.05), (0.25, 0.25, 0.24), GOLD),
    ("IM_R_Shoulder", "r_upperarm", (0, 0, 0.05), (0.25, 0.25, 0.24), GOLD),
    ("IM_L_Forearm", "l_forearm", (0, 0, 0), (0.17, 0.19, 0.32), RED),
    ("IM_R_Forearm", "r_forearm", (0, 0, 0), (0.17, 0.19, 0.32), RED),
    ("IM_L_Thigh", "l_thigh", (0, 0, 0), (0.22, 0.22, 0.38), RED),
    ("IM_R_Thigh", "r_thigh", (0, 0, 0), (0.22, 0.22, 0.38), RED),
    ("IM_L_Shin", "l_calf", (0, 0, 0), (0.20, 0.23, 0.34), GOLD),
    ("IM_R_Shin", "r_calf", (0, 0, 0), (0.20, 0.23, 0.34), GOLD),
]

made = 0
for name, key, offset, dims, mat in parts:
    pos = at(key, offset)
    if pos is not None and plate(name, pos, dims, mat, key):
        made += 1

if made < 5:
    raise RuntimeError(
        "Too few armor plates were placed. The template skeleton bone names "
        "do not match expected GTA-style aliases; inspect the armature and edit ALIASES."
    )

# Export armature, original body and overlay together as an intermediate FBX.
bpy.ops.object.select_all(action="DESELECT")
for obj in bpy.context.scene.objects:
    if obj.type in {"ARMATURE", "MESH"}:
        obj.select_set(True)
bpy.context.view_layer.objects.active = arm
bpy.ops.export_scene.fbx(
    filepath=OUT,
    use_selection=True,
    object_types={"ARMATURE", "MESH"},
    add_leaf_bones=False,
    bake_anim=False,
    path_mode="COPY",
)
print("Wrote intermediate FBX:", OUT, "| armor plates:", made)
