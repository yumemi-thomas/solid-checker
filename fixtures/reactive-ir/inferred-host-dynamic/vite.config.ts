const mode = process.env.NODE_ENV;
export default mode === "production" ? {plugins: []} : {...{server: {port: 3000}}, plugins: []};
