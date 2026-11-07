window.addEventListener("resize", () => console.log("resized"));
window.scrollTo(0, 0);
window.getComputedStyle(document.body);
window.onresize = null;
function tick() {
  console.log("tick");
}
setTimeout(tick, 0);
setTimeout(function () {
  console.log("later");
}, 0);
export {};
