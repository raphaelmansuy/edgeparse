#!/usr/bin/env python3
"""WCAG 2.1 AA contrast ratio audit for EdgeParse site colors."""

def hex_to_rgb(h):
    h = h.lstrip('#')
    return tuple(int(h[i:i+2], 16) for i in (0, 2, 4))

def relative_luminance(r, g, b):
    srgb = [c / 255.0 for c in (r, g, b)]
    lin = [(c / 12.92 if c <= 0.04045 else ((c + 0.055) / 1.055) ** 2.4) for c in srgb]
    return 0.2126 * lin[0] + 0.7152 * lin[1] + 0.0722 * lin[2]

def contrast_ratio(fg, bg):
    l1 = relative_luminance(*hex_to_rgb(fg))
    l2 = relative_luminance(*hex_to_rgb(bg))
    lighter = max(l1, l2)
    darker = min(l1, l2)
    return (lighter + 0.05) / (darker + 0.05)

colors = {
    'white': '#FFFFFF',
    'gray-1': '#F8FAFC',
    'gray-2': '#E2E8F0',
    'gray-3': '#94A3B8',
    'gray-4': '#475569',
    'gray-5': '#1E293B',
    'gray-6': '#0F172A',
    'black': '#020617',
    'accent': '#2563EB',
    'accent-dark': '#3B82F6',
    'accent-high-dark': '#60A5FA',
    'rust': '#F97316',
    'python': '#3776AB',
    'nodejs': '#339933',
    'benchmark-win': '#16A34A',
}

combos = [
    # DARK MODE
    ('hero-subtitle (gray-3 on gray-6)', 'gray-3', 'gray-6'),
    ('section-subtitle (gray-3 on gray-6)', 'gray-3', 'gray-6'),
    ('feature-desc (gray-3 on gray-5)', 'gray-3', 'gray-5'),
    ('problem-desc (gray-3 on gray-6)', 'gray-3', 'gray-6'),
    ('benchmark-th (gray-3 on gray-6)', 'gray-3', 'gray-6'),
    ('benchmark-note (gray-3 on gray-6)', 'gray-3', 'gray-6'),
    ('cta-subtitle (gray-3 on gray-6)', 'gray-3', 'gray-6'),
    ('tab-btn (gray-3 on black)', 'gray-3', 'black'),
    ('hero-title (gray-1 on gray-6)', 'gray-1', 'gray-6'),
    ('text-accent (accent-dark on gray-6)', 'accent-dark', 'gray-6'),
    ('btn-primary (white on accent)', 'white', 'accent'),
    ('btn-secondary (gray-1 on gray-6)', 'gray-1', 'gray-6'),
    ('install-code (gray-2 on gray-5)', 'gray-2', 'gray-5'),
    ('code-panel (gray-2 on gray-6)', 'gray-2', 'gray-6'),
    ('feature-link (accent-dark on gray-5)', 'accent-dark', 'gray-5'),
    ('stage-crate (rust on gray-6)', 'rust', 'gray-6'),
    ('bar-label (white on accent)', 'white', 'accent'),
    ('bar-label (white on gray-3)', 'white', 'gray-3'),
    ('highlight-td (accent-dark on gray-6)', 'accent-dark', 'gray-6'),
    ('json-label (gray-3 on gray-6)', 'gray-3', 'gray-6'),
    ('pdf-meta (gray-3 on gray-6)', 'gray-3', 'gray-6'),
    # LIGHT MODE
    ('LM: subtitle (gray-3 on white)', 'gray-3', 'white'),
    ('LM: gray-3 on gray-1', 'gray-3', 'gray-1'),
    ('LM: gray-4 on white', 'gray-4', 'white'),
    ('LM: accent on white', 'accent', 'white'),
    ('LM: accent-dark on white', 'accent-dark', 'white'),
    ('LM: python on white', 'python', 'white'),
    ('LM: nodejs on white', 'nodejs', 'white'),
    ('LM: benchmark-win on white', 'benchmark-win', 'white'),
]

print(f"{'Combination':<50} {'Ratio':>6}  {'AA-Norm':>8}  {'AA-Lrg':>7}")
print('-' * 78)
fails = []
for label, fg_key, bg_key in combos:
    fg = colors[fg_key]
    bg = colors[bg_key]
    r = contrast_ratio(fg, bg)
    aa_n = 'PASS' if r >= 4.5 else 'FAIL'
    aa_l = 'PASS' if r >= 3.0 else 'FAIL'
    m = ' ❌' if aa_n == 'FAIL' else ''
    print(f'{label:<50} {r:>6.2f}  {aa_n:>8}  {aa_l:>7}{m}')
    if aa_n == 'FAIL':
        fails.append((label, r, fg, bg))

print()
print(f'Total: {len(combos)} | Failures: {len(fails)}')
for label, r, fg, bg in fails:
    print(f'  ❌ {label}: ratio {r:.2f} ({fg} on {bg})')
