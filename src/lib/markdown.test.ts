// @vitest-environment jsdom
import { expect, it } from 'vitest'
import { renderMarkdown } from './markdown'

it('renders Markdown structure and removes executable HTML and unsafe links', () => {
  const root = document.createElement('div')
  root.innerHTML = renderMarkdown([
    '# 标题', '', '**重点**', '', '- 项目', '', '> 引用', '',
    '| 名称 | 值 |', '| --- | --- |', '| 项目 | 1 |', '',
    '```ts', 'const value = "<script>"', '```', '',
    '[链接](https://example.com)', '',
    '<script>alert(1)</script><img src="x" onerror="alert(1)">',
    '<a href="javascript:alert(1)">危险链接</a>',
  ].join('\n'))
  expect(root.querySelector('h1')?.textContent).toBe('标题')
  expect(root.querySelector('strong')?.textContent).toBe('重点')
  expect(root.querySelector('li')?.textContent).toBe('项目')
  expect(root.querySelector('blockquote')?.textContent?.trim()).toBe('引用')
  expect(root.querySelector('td')?.textContent).toBe('项目')
  expect(root.querySelector('pre code.language-ts')?.textContent).toContain('"<script>"')
  expect(root.querySelector('a')?.getAttribute('href')).toBe('https://example.com')
  expect(root.querySelector('script, [onerror], [href^="javascript:"]')).toBeNull()
})
