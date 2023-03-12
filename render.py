import bpy

import sys
argv = sys.argv
argv = argv[argv.index("--") + 1:]

prefix = argv[0]
idx = int(argv[1])

infile = "%s/gcode/gcode_%03d.obj" % (prefix, idx)
outfile = "%s/gcode/render/gcode_%03d.png" % (prefix, idx)

bpy.context.scene.render.filepath = outfile
bpy.ops.import_scene.obj(filepath=infile, axis_forward="Y", axis_up="Z")
ob = bpy.context.selected_objects[0]
ob.data.materials[0] = bpy.data.materials["red"]
bpy.ops.render.render(write_still=True)
bpy.ops.object.delete()
