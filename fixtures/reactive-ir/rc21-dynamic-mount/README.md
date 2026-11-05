# `<Dynamic>` mounts with no strict window (ADR 0662)

An exclusive mount has no window, so SC5001 is silent; with no boundary SC5003 stays open,
blocker `mount-form-window`. A one-step alias answers the same. A mixed mount keeps SC5001
uncertifiable. A tag mount keeps its violation.
