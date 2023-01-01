import bpy

import sys
argv = sys.argv
argv = argv[argv.index("--") + 1:]

prefix = argv[0]
idx = argv[1]

infile = "{}/gcode/gcode_{}.obj".format(prefix, idx)
outfile = "{}/gcode/render/gcode_{}.png".format(prefix, idx)

bpy.context.scene.render.filepath = outfile
bpy.ops.import_scene.obj(filepath=infile, axis_forward="X", axis_up="Z")
ob = bpy.context.selected_objects[0]
ob.data.materials[0] = bpy.data.materials["red"]
bpy.ops.render.render(write_still=True)
bpy.ops.object.delete()
