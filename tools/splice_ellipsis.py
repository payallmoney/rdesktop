# -*- coding: utf-8 -*-
"""长文件名省略:禁自动换行(默认 WRAP 会把长名折到不可见行) + trimming 画 ... """
import io

p = 'src/app.rs'
s = io.open(p, encoding='utf-8').read()
old = """            // 图标下方名称:居中
            let _ = fmt_label.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_CENTER);"""
new = """            // 图标下方名称:居中;禁自动换行(默认 WRAP 会把长名折到不可见的第二行,
            // 表现为只露第一个字),超宽交给 trimming 画"..."省略
            let _ = fmt_label.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_CENTER);
            let _ = fmt_label.SetWordWrapping(windows::Win32::Graphics::DirectWrite::DWRITE_WORD_WRAPPING_NO_WRAP);
            let _ = fmt_title.SetWordWrapping(windows::Win32::Graphics::DirectWrite::DWRITE_WORD_WRAPPING_NO_WRAP);"""
assert s.count(old) == 1
s = s.replace(old, new)
io.open(p, 'w', encoding='utf-8', newline='').write(s)
print('app.rs ok')

p2 = 'src/render.rs'
s2 = io.open(p2, encoding='utf-8').read()
old2 = """            &DWRITE_TRIMMING {
                granularity: DWRITE_TRIMMING_GRANULARITY_CHARACTER,
                delimiter: 0,
                delimiterCount: 0,
            },"""
new2 = """            &DWRITE_TRIMMING {
                granularity: DWRITE_TRIMMING_GRANULARITY_CHARACTER,
                delimiter: b'.' as u32, // 三个点 = "..."(0 表示不画省略号,之前是硬切)
                delimiterCount: 3,
            },"""
assert s2.count(old2) == 1
s2 = s2.replace(old2, new2)
io.open(p2, 'w', encoding='utf-8', newline='').write(s2)
print('render.rs ok')
