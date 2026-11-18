# store-setter-draft-assign-proxy

ADR 1067 negative: an arbitrary seed Proxy can forge rc.21's private target
protocol and forward updater writes to `Number`. A genuine setter alone proves
no allocation; this probe must stay uncertifiable. The rc.21 source probe
confirmed the updater receives this seed unchanged and can replace isFinite.
