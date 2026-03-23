#!/usr/bin/env python3
"""Test proposed contrast fixes."""
import sys

def hex_to_rgb(h):
    h = h.lstrip('#')
    return tuple(int(h[i:i+2], 16) for i in (0, 2, 4))

def lum(r, g, b):
    s = [c / 255.0 for c in (r, g, b)]
    l = [(c / 12.92 if c <= 0.04045 else ((c + 0.055) / 1.055) ** 2.4) for c in s]
    return 0.2126 * l[0] + 0.7152 * l[1] + 0.0722 * l[2]

def cr(fg, bg):
    l1 = lum(*hex_to_rgb(fg))
    l2 = lum(*hex_to_rgb(bg))
    return (max(l1, l2) + 0.05) / (min(l1, l2) + 0.05)

fixes = [
    ('#60A5FA on #1E293B', '#60A5FA', '#1E293B'),
    ('white on #64748B', '#FFFFFF', '#64748B'),
    ('#64748B on white', '#64748B', '#FFFFFF'),
    ('#2563EB on white', '#2563EB', '#FFFFFF'),
    ('#15803D on white', '#15803D', '#FFFFFF'),
    ('#166534 on white', '#166534', '#FFFFFF'),
]

for label, fg, bg in fixes:
    r = cr(fg, bg)
    ok = 'PASS' if r >= 4.5 else 'FAIL'
    sys.stdout.write(f'{label:30s} {r:.2f} {ok}\n')
