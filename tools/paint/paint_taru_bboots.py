"""Bounding Boots, Tarutaru (64x64 texels). Same design as the Hume Bounding Boots
(saddle leather shaft, leather ladder buckle with dark slots, tan knee tab with rivets,
dark leather wrap, tan toe and heel) on the Taru layout, read off the Taru original."""
import sys, os
import numpy as np
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from paint import Canvas, PAL, LEATHER, mix, rounded_rect
from paint_bboots import SADDLE, TAN, DARK, SOLE, welt

def paint(alpha):
    c = Canvas(64, 64, SADDLE[1], base_mat=LEATHER)
    # shaft: saddle leather, creases, a stitched seam across
    shaft = c.rect(-2, -2, 66, 66)
    c.shade(shaft, SADDLE, -2, 66, light=0.1, shadow=0.1, ink=False, mat=LEATHER)
    for x in (36, 46, 56):
        c.paint(c.poly_mask([(x - 0.25, 26), (x + 0.25, 26), (x + 0.8, 50), (x + 0.3, 50)]), SADDLE[0], LEATHER)
    welt(c, -2, 30.6, 66)
    # dark top rim on the back of the shaft
    c.shade(c.rect(32, -2, 66, 19.6), DARK, -2, 19.6, light=0.25, shadow=0.25, ink=False, mat=LEATHER)
    c.stitch(33, 18.6, 65, 18.6)
    # dark leather wrap band under the buckle
    c.shade(c.rect(-2, 19.6, 33, 24), DARK, 19.6, 24, light=0.2, shadow=0.2, ink=False, mat=LEATHER)
    # ladder buckle at the front: boot leather frame with four dark slots
    frame = c.rect(-2, 0.4, 24.2, 17.4)
    c.cast(frame, 0.3, 0.8, 0.5)
    c.shade(frame, SADDLE, 0.4, 17.4, light=0.2, shadow=0.18, ink=False, mat=LEATHER)
    c.paint(c.rect(-2, 0.4, 24.2, 1.6), TAN[1], LEATHER); c.paint(c.rect(-2, 16.2, 24.2, 17.4), TAN[0], LEATHER)
    for x0 in (1.4, 7.2, 13, 18.8):
        slot = c.rect(x0, 3.2, x0 + 3.6, 14.4)
        c.paint(slot, SOLE[1], LEATHER); c.outline(slot, width=0.3)
        c.paint(c.rect(x0, 13.3, x0 + 3.6, 14.4), TAN[0], LEATHER)
    c.outline(frame, width=0.3)
    # strap end and rivet beside the buckle
    c.strap(24.2, -2, 32, 19.6, tones=TAN, cast=True); c.rivet(27.6, 6.8, 1.0)
    # tan knee tab with rivets
    tab = c.poly_mask(rounded_rect(8, 23.6, 28.6, 38.6, 2.6))
    c.cast(tab, 0.4, 0.8, 0.5)
    c.shade(tab, TAN, 23.6, 38.6, light=0.22, shadow=0.18, ink=False, mat=LEATHER)
    c.stitch(9.4, 25, 27.2, 25); c.stitch(9.4, 37.2, 27.2, 37.2); c.outline(tab, width=0.3)
    c.rivet(12.2, 28.4, 0.9); c.rivet(24.6, 28.6, 0.9)
    # dark lining strip down the front
    lin = c.rect(16, 38.6, 22.4, 56)
    c.shade(lin, DARK, 38.6, 56, light=0.1, shadow=0.1, ink=False, mat=LEATHER); c.outline(lin, width=0.3)
    # tan toe and heel at the bottom, dark soles
    toe = c.poly_mask([(-2, 51.6), (14, 51.6), (17, 55), (17, 66), (-2, 66)])
    c.cast(toe, 0.3, 0.8, 0.5); c.shade(toe, TAN, 51.6, 66, light=0.22, shadow=0.15, ink=False, mat=LEATHER)
    c.stitch(0, 53.2, 13.5, 53.2); c.outline(toe, width=0.3)
    heel = c.poly_mask([(51, 66), (51, 56), (54, 52.4), (66, 52.4), (66, 66)])
    c.cast(heel, 0.3, 0.8, 0.5); c.shade(heel, TAN, 52.4, 66, light=0.2, shadow=0.15, ink=False, mat=LEATHER)
    c.outline(heel, width=0.3)
    c.shade(c.rect(17, 60, 51, 66), SOLE, 60, 66, ink=False, mat=LEATHER)
    return c.result(alpha)
