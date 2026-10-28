// Stand-in for window.__TAURI__ when the UI is opened in a normal browser (design work only).
const listeners = {};
const emit = (name, payload) => (listeners[name] || []).forEach((cb) => cb({ payload }));

const SAMPLE = [
  ["C:\\Photos\\IMG_2041.HEIC", "image", 3_400_000],
  ["C:\\Photos\\beach.png", "image", 8_100_000],
  ["C:\\Videos\\holiday.mkv", "video", 920_000_000],
  ["C:\\Music\\live-session.flac", "audio", 41_000_000],
  ["C:\\Work\\config.yaml", "data", 4_200],
  ["C:\\Work\\notes.md", "document", 12_000],
];
const TARGETS = {
  image: ["jpg", "png", "webp", "avif", "gif", "bmp", "tiff", "ico", "tga", "qoi", "pdf"],
  video: ["mp4", "mkv", "webm", "mov", "avi", "gif", "mp3", "m4a", "wav", "flac", "ogg", "opus"],
  audio: ["mp3", "m4a", "wav", "flac", "ogg", "opus", "aiff"],
  data: ["json", "yaml", "toml", "csv"],
  document: ["html"],
};
const DEFAULT = { image: "jpg", video: "mp4", audio: "mp3", data: "json", document: "html" };

let cancel = false;

async function invoke(cmd, args) {
  if (cmd === "inspect") {
    return {
      skipped: 0,
      files: SAMPLE.map(([path, kind, size]) => ({
        path,
        name: path.split("\\").pop(),
        ext: path.split(".").pop().toLowerCase(),
        size,
        kind,
        targets: TARGETS[kind],
        default_target: DEFAULT[kind],
      })),
    };
  }
  if (cmd === "thumbnail") return null;
  if (cmd === "cancel") return void (cancel = true);
  if (cmd === "convert") {
    cancel = false;
    const start = Date.now();
    let done = 0;
    let canceled = 0;
    (async () => {
      for (const job of args.jobs) {
        if (cancel) {
          emit("job", { id: job.id, state: "canceled", progress: 0 });
          canceled++;
          continue;
        }
        for (let p = 0; p <= 1; p += 0.1) {
          emit("job", { id: job.id, state: "working", progress: p });
          await new Promise((r) => setTimeout(r, 60));
        }
        emit("job", { id: job.id, state: "done", progress: 1, output: job.path.replace(/\.\w+$/, "." + job.target), outSize: 1_200_000 });
        done++;
      }
      emit("batch", { done, failed: 0, canceled, millis: Date.now() - start });
    })();
    return;
  }
}

export default {
  core: { invoke },
  event: {
    listen: async (name, cb) => {
      (listeners[name] ||= []).push(cb);
      return () => {};
    },
  },
  dialog: { open: async ({ directory }) => (directory ? "C:\\Converted" : ["C:\\sample"]) },
  opener: { revealItemInDir: async () => {} },
  webview: { getCurrentWebview: () => ({ onDragDropEvent: async () => () => {} }) },
};
