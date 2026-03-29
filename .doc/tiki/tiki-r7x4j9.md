---
title: Fix WASD movement to be relative to camera direction
type: story
status: done
priority: 3
points: 5
---
The current WASD implementation moves relative to the camera direction using forward/right vectors from yaw/pitch. Movement should use world-space axes instead (W = +Z, S = -Z, D = +X, A = -X) so it doesn't depend on which way the camera is facing.
