// The compiler extracts functions, function expressions and block-bodied
// arrows. Object methods, accessors and class methods keep their directive and
// rc.17 rejects them with a compile error — finding on the directive.
export const api = {
  async getUser(id: string) {
    "use server";
    return id;
  },
  get user() {
    "use server";
    return 1;
  },
  set user(_value: number) {
    "use server";
  },
};

export class Api {
  constructor() {
    "use server";
  }
  async getUser(id: string) {
    "use server";
    return id;
  }
  get size() {
    "use server";
    return 1;
  }
}

// Negative: function-valued properties and class fields are extracted.
export const eligible = {
  getUser: async function (id: string) {
    "use server";
    return id;
  },
  arrow: async (id: string) => {
    "use server";
    return id;
  },
};

export class Field {
  load = async (id: string) => {
    "use server";
    return id;
  };
}
