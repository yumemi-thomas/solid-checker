const sharedConfig = { load: () => 1 };
sharedConfig.load = () => 2;
export { sharedConfig };
