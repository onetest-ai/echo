# raw_inbox

Drop sources here exactly as they came: documents, slides, PDFs, exports, transcripts, screenshots.
Nothing here is ever edited or deleted — the pages in `knowledge/` point back to these files.

Then ask Claude: `/process-inbox`. Each file becomes a page (or updates one), gets a row in
`knowledge/CATALOG.md`, and a `processed` line in `log.md`.

Claude reads Markdown, text, PDF and images directly. For Word or PowerPoint files, save a PDF copy here too.
