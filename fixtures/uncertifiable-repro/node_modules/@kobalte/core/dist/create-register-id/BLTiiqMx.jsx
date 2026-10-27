//#region src/primitives/create-register-id/create-register-id.ts
/**
* Create a function that call the setter with an id and return a function to reset it.
*/
function createRegisterId(setter) {
	return (id) => {
		setter(id);
		return () => setter(void 0);
	};
}
//#endregion
export { createRegisterId as t };
