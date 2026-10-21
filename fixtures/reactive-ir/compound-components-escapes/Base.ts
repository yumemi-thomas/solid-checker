export class Base {
  static set List(member: () => unknown) {
    member();
  }
}
