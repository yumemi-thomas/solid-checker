# Reactive callback arguments

The exported helper invokes its callback inline and supplies a newly created
accessor at argument position 1. Contract generation must describe that
argument without treating the accessor as an uncaptured source read.
