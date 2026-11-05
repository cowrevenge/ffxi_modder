"""Centurion's Greaves, Tarutaru (64x64 texels), Bastok Pride. Same design as the Hume
greaves (indigo padded panels, cool leather strap harness, broad shin strap with a
steel buckle, steel shin plate, creased leather calf) on the Taru layout, read off
the Taru original."""
import sys, os
import numpy as np
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from paint import Canvas, PAL, DARK_STEEL, LEATHER, CLOTH, mix, rounded_rect, use_cool_leather
use_cool_leather()
from paint_feet import panel, PAD

def paint(alpha):
    c = Canvas(64, 64, PAL['leather'][1], base_mat=LEATHER)
    am = c.alpha(alpha)
    c.shade(c.rect(-2, -2, 66, 66), PAL['leather'], -2, 66, light=0.05, shadow=0.05, ink=False, mat=LEATHER)
    # calf: plain creased leather at the back, dark heel band at the bottom
    calf = c.rect(41, 23.6, 66, 66)
    c.shade(calf, PAL['leather'], 23.6, 66, light=0.12, shadow=0.15, ink=False, mat=LEATHER)
    for x in (47, 53, 60):
        c.paint(c.poly_mask([(x - 0.25, 26), (x + 0.25, 26), (x + 0.9, 55), (x + 0.4, 55)]), PAL['leather'][0], LEATHER)
    c.shade(c.rect(41, 57, 66, 66), [PAL['leather'][0] * 0.5, PAL['leather'][0] * 0.75, PAL['leather'][0]], 57, 66, ink=False, mat=LEATHER)
    # padded indigo panels, top row and lower leg
    for x0, x1 in ((4, 12), (16, 28), (32, 40), (44, 56.5)):
        panel(c, x0, -2, x1, 17.2)
    for x0, x1, y1 in ((4, 12, 66), (16, 28, 49), (32, 40, 66)):
        panel(c, x0, 23.6, x1, y1)
    # vertical straps over the panel edges
    for x0, x1, y0, y1 in ((-2, 4, -2, 66), (12, 16, -2, 66), (28, 32, -2, 66), (40, 44, -2, 17.2), (40, 41.5, 23.6, 66), (56.5, 66, -2, 17.2)):
        c.strap(x0, y0, x1, y1, stitch=(x1 - x0) > 3.5, cast=True)
    # broad strap round the top of the shin, steel buckle, a rivet
    top = c.rect(-2, 17.2, 66, 23.6)
    c.cast(top, 0, 0.8, 0.5)
    c.shade(top, PAL['leather'], 17.2, 23.6, light=0.22, shadow=0.22, ink=False, mat=LEATHER)
    c.stitch(-1, 18.1, 65, 18.1); c.stitch(-1, 22.7, 65, 22.7); c.outline(top, width=0.3)
    fr = c.rect(40.6, 18.2, 45, 23); c.cast(fr, 0.25, 0.6, 0.5); c.metal(fr, 18.2, 23, 40.6, 45, glint=False)
    slot = c.rect(41.6, 19.3, 44, 21.9); c.paint(slot, PAL['leather'][0], LEATHER); c.outline(slot, width=0.25)
    c.rivet(52.5, 20.4, 0.9)
    # steel wing on the shin strap
    wing = c.poly_mask([(47.5, 11.2), (54.5, 11.2), (54.5, 15.4), (51, 16.6), (47.5, 15.4)]) & am
    c.cast(wing, 0.3, 0.6, 0.5); c.metal(wing, 11.2, 16.6, 47.5, 54.5, glint=False); c.rivet(51, 13.2, 0.7)
    # shin plate: winged steel, dark steel top rail with two rivets (cut-outs give the wings)
    plate = c.rect(13.5, 48, 31, 66) & am
    c.cast(plate, 0.3, 0.8, 0.45); c.metal(plate, 48, 66, 13.5, 31)
    rail = c.rect(13.5, 48, 31, 52) & am
    c.metal(rail, 48, 52, 13.5, 31, tones=DARK_STEEL, glint=False)
    c.rivet(14.8, 49.6, 0.7); c.rivet(25, 50, 0.7)
    c.cut_outline(alpha)
    return c.result(alpha)
