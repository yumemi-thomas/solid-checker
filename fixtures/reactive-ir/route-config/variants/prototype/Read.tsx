// An inherited `transformUrl`: the router calls `config.transformUrl(...)`
// with the config as `this`, which reaches `this.routes`.
(Object.prototype as unknown as { transformUrl?: (url: string) => string }).transformUrl =
  function (url: string) {
    return url;
  };
