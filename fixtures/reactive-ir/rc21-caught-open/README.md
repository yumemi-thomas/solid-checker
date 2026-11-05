# SC5008: a caught read no call shows (ADR 0658)

In a compute, a prop's Get, a handed value's call and a literal's getter may throw a
pending read into the `catch`: uncertifiable. A plain object's member stays silent.
