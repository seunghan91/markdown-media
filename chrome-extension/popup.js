import init, {
  convert_to_markdown,
  convert_to_json,
  detect_format,
  get_version,
} from './mdm_core.js';

const dropzone = document.getElementById('dropzone');
const fileInput = document.getElementById('file-input');
const status = document.getElementById('status');
const result = document.getElementById('result');
const resultTitle = document.getElementById('result-title');
const resultMeta = document.getElementById('result-meta');
const output = document.getElementById('output');
const versionEl = document.getElementById('version');

let ready = false;

// Init WASM
(async () => {
  try {
    await init();
    ready = true;
    versionEl.textContent = `v${get_version()} · 오프라인 변환`;
    versionEl.style.color = '#3fb950';
  } catch (e) {
    versionEl.textContent = 'WASM 로드 실패';
    versionEl.style.color = '#f85149';
    console.error(e);
  }
})();

// Drag & drop
['dragenter', 'dragover'].forEach(ev => {
  dropzone.addEventListener(ev, e => { e.preventDefault(); dropzone.classList.add('dragover'); });
});
['dragleave', 'drop'].forEach(ev => {
  dropzone.addEventListener(ev, e => { e.preventDefault(); dropzone.classList.remove('dragover'); });
});
dropzone.addEventListener('drop', e => {
  const file = e.dataTransfer.files[0];
  if (file) convert(file);
});
fileInput.addEventListener('change', e => {
  const file = e.target.files[0];
  if (file) convert(file);
  fileInput.value = '';
});

async function convert(file) {
  if (!ready) {
    showStatus('WASM 엔진 로딩 중... 잠시 후 다시 시도', 'error');
    return;
  }

  showStatus(`${file.name} 변환 중...`, '');
  result.classList.remove('visible');

  const t0 = performance.now();

  try {
    const data = new Uint8Array(await file.arrayBuffer());
    const format = detect_format(data, file.name);

    let markdown;
    try {
      const jsonStr = convert_to_json(data, file.name);
      const parsed = JSON.parse(jsonStr);
      markdown = parsed.markdown;
    } catch {
      markdown = convert_to_markdown(data, file.name);
    }

    const elapsed = ((performance.now() - t0) / 1000).toFixed(2);
    const lines = markdown.split('\n').length;
    const chars = markdown.length;

    showStatus(`변환 완료 · ${elapsed}s`, 'success');

    resultTitle.textContent = file.name.replace(/\.[^.]+$/, '.md');
    resultMeta.textContent = `${format.toUpperCase()} · ${(file.size / 1024).toFixed(1)}KB → ${lines}줄 · ${chars.toLocaleString()}자`;
    output.value = markdown;
    result.classList.add('visible');

    // Store for download
    output.dataset.filename = file.name.replace(/\.[^.]+$/, '.md');
  } catch (e) {
    showStatus(`변환 실패: ${e.message || e}`, 'error');
    console.error(e);
  }
}

function showStatus(msg, type) {
  status.textContent = msg;
  status.className = 'status visible' + (type ? ' ' + type : '');
}

// Copy
document.getElementById('btn-copy').addEventListener('click', () => {
  navigator.clipboard.writeText(output.value).then(() => {
    const btn = document.getElementById('btn-copy');
    btn.textContent = '✅';
    setTimeout(() => btn.textContent = '📋', 1200);
  });
});

// Download
document.getElementById('btn-download').addEventListener('click', () => {
  const blob = new Blob([output.value], { type: 'text/markdown' });
  const url = URL.createObjectURL(blob);
  const a = document.createElement('a');
  a.href = url;
  a.download = output.dataset.filename || 'converted.md';
  a.click();
  URL.revokeObjectURL(url);
});
