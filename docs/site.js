const components = [
  "answer-highlight",
  "bottom-up-letters",
  "carousel-3d",
  "channel-thread",
  "check-cycle",
  "cinematic-text-swap",
  "circle-marker",
  "confetti",
  "crossed-off",
  "cursor-flyover",
  "echo-stack",
  "glitch-cycle",
  "inflating-text",
  "lines-to-text",
  "logo-assemble",
  "multi-text-typewriter",
  "per-character-rise",
  "period-drop",
  "prompt-zoom",
  "reel-collage",
  "short-slide-right",
  "slide-in-text",
  "soft-blur-in",
  "spinning-text-wheel",
  "spring-scale-in",
  "squeeze-in",
  "staggered-fade-up",
  "strike-through",
  "text-build",
  "text-highlight",
  "text-marker",
  "text-messages",
  "text-reveal",
  "text-rewrite",
  "text-swap",
  "text-swell",
  "top-down-letters",
  "tracking-in",
  "type-writer",
  "typing-code-block",
  "word-by-word",
  "word-flip",
  "word-gather",
  "youtube-comment-highlight",
  "youtube-end-card",
  "youtube-subscribe-nudge",
];

const grid = document.querySelector("#video-grid");
const count = document.querySelector("#component-count");
const copyButton = document.querySelector("#copy-command");
const copyStatus = document.querySelector("#copy-status");
const installCommand = document.querySelector("#install-command");

count.textContent = String(components.length);

const cards = components.map((name, index) => {
  const article = document.createElement("article");
  article.className = "video-card";
  article.style.setProperty("--index", String(index));

  const frame = document.createElement("div");
  frame.className = "video-frame";

  const video = document.createElement("video");
  video.src = `videos/${name}.mp4`;
  video.preload = "metadata";
  video.muted = true;
  video.loop = true;
  video.playsInline = true;
  video.controls = true;
  video.setAttribute("aria-label", `${name} component preview`);
  video.addEventListener("loadeddata", () => frame.classList.add("ready"), { once: true });
  video.addEventListener("error", () => frame.classList.add("error"), { once: true });

  const meta = document.createElement("div");
  meta.className = "video-meta";
  const title = document.createElement("h3");
  title.textContent = name;
  const feature = document.createElement("span");
  feature.textContent = `feature = “${name}”`;
  meta.append(title, feature);
  frame.append(video);
  article.append(frame, meta);
  grid.append(article);

  return { article, video };
});

if ("IntersectionObserver" in window) {
  const observer = new IntersectionObserver(
    (entries) => {
      for (const entry of entries) {
        const card = cards.find(({ article }) => article === entry.target);
        if (!card) continue;
        if (entry.isIntersecting) {
          card.video.play().catch(() => {});
        } else {
          card.video.pause();
        }
      }
    },
    { rootMargin: "160px 0px", threshold: 0.25 },
  );
  cards.forEach(({ article }) => observer.observe(article));
}

copyButton.addEventListener("click", async () => {
  try {
    await navigator.clipboard.writeText(installCommand.textContent);
    copyButton.textContent = "Copied";
    copyStatus.textContent = "Install command copied to the clipboard.";
  } catch {
    copyStatus.textContent = "Select the command and copy it manually.";
  }

  window.setTimeout(() => {
    copyButton.textContent = "Copy";
    copyStatus.textContent = "";
  }, 2400);
});
