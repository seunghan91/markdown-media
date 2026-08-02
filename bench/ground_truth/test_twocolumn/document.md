---
format: pdf
version: "1.4"
pages: 1
images: 0
fonts: 2
tables: 0
title: "Two-Column Layout Test"
author: "(anonymous)"
---

# Two-Column Document

Left column paragraph one. This text should appear in the left column of the document. Proper reading order detection should read this column completely before moving to the right column.

Left column paragraph two continues with more content.

Right column paragraph one. This text should appear in the right column. If the parser reads left-to-right line by line instead of column by column, the text will be garbled.

Right column paragraph two has additional information.

## After Columns

This section appears after the two-column layout and should be detected as full-width content.
