#!/usr/bin/env bash


seq 1 100 | xargs -P8 -n1 "$BLENDER" -b tdp.blend --background --python render.py -- $PWD
