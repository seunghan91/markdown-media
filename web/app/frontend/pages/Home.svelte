<script lang="ts">
  let url = ''
  let file: File | null = null
  let loading = false
  let result = ''
  let error = ''
  let dragActive = false

  async function convertUrl() {
    if (!url.trim()) return

    loading = true
    error = ''
    result = ''

    try {
      const response = await fetch('/api/convert/url', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ url })
      })

      const data = await response.json()

      if (response.ok) {
        result = data.markdown
      } else {
        error = data.error || 'Failed to convert URL'
      }
    } catch (e) {
      error = 'Network error occurred'
    } finally {
      loading = false
    }
  }

  async function convertFile() {
    if (!file) return

    loading = true
    error = ''
    result = ''

    try {
      const formData = new FormData()
      formData.append('file', file)

      const response = await fetch('/api/convert/file', {
        method: 'POST',
        body: formData
      })

      const data = await response.json()

      if (response.ok) {
        result = data.markdown
      } else {
        error = data.error || 'Failed to convert file'
      }
    } catch (e) {
      error = 'Network error occurred'
    } finally {
      loading = false
    }
  }

  function handleDrop(e: DragEvent) {
    e.preventDefault()
    dragActive = false

    const droppedFile = e.dataTransfer?.files[0]
    if (droppedFile && droppedFile.type === 'application/pdf') {
      file = droppedFile
    }
  }

  function handleDragOver(e: DragEvent) {
    e.preventDefault()
    dragActive = true
  }

  function handleDragLeave() {
    dragActive = false
  }

  function handleFileSelect(e: Event) {
    const input = e.target as HTMLInputElement
    if (input.files?.[0]) {
      file = input.files[0]
    }
  }

  function downloadMarkdown() {
    if (!result) return

    const blob = new Blob([result], { type: 'text/markdown' })
    const url = URL.createObjectURL(blob)
    const a = document.createElement('a')
    a.href = url
    a.download = 'converted.md'
    a.click()
    URL.revokeObjectURL(url)
  }

  function copyToClipboard() {
    navigator.clipboard.writeText(result)
  }
</script>

<div class="min-h-screen bg-gradient-to-br from-gray-50 to-gray-100">
  <!-- Header -->
  <header class="bg-white shadow-sm">
    <div class="max-w-4xl mx-auto px-4 py-6">
      <h1 class="text-2xl font-bold text-gray-900">
        MDM Web
      </h1>
      <p class="text-gray-600 mt-1">URL or PDF to Markdown Converter</p>
    </div>
  </header>

  <main class="max-w-4xl mx-auto px-4 py-8">
    <!-- URL Input Section -->
    <section class="bg-white rounded-2xl shadow-sm p-6 mb-6">
      <h2 class="text-lg font-semibold mb-4">Convert URL</h2>
      <div class="flex gap-3">
        <input
          type="url"
          bind:value={url}
          placeholder="https://example.com/article"
          class="flex-1 px-4 py-3 border border-gray-300 rounded-lg focus:ring-2 focus:ring-primary-500 focus:border-transparent"
          disabled={loading}
        />
        <button
          on:click={convertUrl}
          disabled={loading || !url.trim()}
          class="px-6 py-3 bg-primary-600 text-white rounded-lg font-medium hover:bg-primary-700 disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
        >
          {#if loading}
            <span class="spinner"></span>
          {:else}
            Convert
          {/if}
        </button>
      </div>
    </section>

    <!-- File Upload Section -->
    <section class="bg-white rounded-2xl shadow-sm p-6 mb-6">
      <h2 class="text-lg font-semibold mb-4">Convert PDF</h2>
      <div
        class="drop-zone"
        class:active={dragActive}
        on:drop={handleDrop}
        on:dragover={handleDragOver}
        on:dragleave={handleDragLeave}
        role="button"
        tabindex="0"
      >
        {#if file}
          <div class="flex items-center justify-center gap-2">
            <svg class="w-6 h-6 text-green-500" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 13l4 4L19 7" />
            </svg>
            <span class="font-medium">{file.name}</span>
            <button
              on:click={() => file = null}
              class="text-red-500 hover:text-red-700"
            >
              <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
              </svg>
            </button>
          </div>
        {:else}
          <div class="text-gray-500">
            <svg class="w-12 h-12 mx-auto mb-3 text-gray-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M7 16a4 4 0 01-.88-7.903A5 5 0 1115.9 6L16 6a5 5 0 011 9.9M15 13l-3-3m0 0l-3 3m3-3v12" />
            </svg>
            <p>Drag and drop a PDF file here, or</p>
            <label class="cursor-pointer text-primary-600 hover:text-primary-700">
              browse files
              <input type="file" accept=".pdf" on:change={handleFileSelect} class="hidden" />
            </label>
          </div>
        {/if}
      </div>
      {#if file}
        <button
          on:click={convertFile}
          disabled={loading}
          class="mt-4 w-full px-6 py-3 bg-primary-600 text-white rounded-lg font-medium hover:bg-primary-700 disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
        >
          {#if loading}
            <span class="spinner mx-auto"></span>
          {:else}
            Convert PDF
          {/if}
        </button>
      {/if}
    </section>

    <!-- Error Message -->
    {#if error}
      <div class="bg-red-50 border border-red-200 text-red-700 px-4 py-3 rounded-lg mb-6">
        {error}
      </div>
    {/if}

    <!-- Result Section -->
    {#if result}
      <section class="bg-white rounded-2xl shadow-sm p-6">
        <div class="flex items-center justify-between mb-4">
          <h2 class="text-lg font-semibold">Result</h2>
          <div class="flex gap-2">
            <button
              on:click={copyToClipboard}
              class="px-4 py-2 text-sm border border-gray-300 rounded-lg hover:bg-gray-50 transition-colors"
            >
              Copy
            </button>
            <button
              on:click={downloadMarkdown}
              class="px-4 py-2 text-sm bg-gray-900 text-white rounded-lg hover:bg-gray-800 transition-colors"
            >
              Download .md
            </button>
          </div>
        </div>
        <pre class="bg-gray-900 text-gray-100 p-4 rounded-lg overflow-x-auto text-sm max-h-96 overflow-y-auto"><code>{result}</code></pre>
      </section>
    {/if}
  </main>

  <!-- Footer -->
  <footer class="max-w-4xl mx-auto px-4 py-8 text-center text-gray-500 text-sm">
    <p>MDM Web - Part of the Markdown Media project</p>
  </footer>
</div>
