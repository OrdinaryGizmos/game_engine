use rapier3d::prelude::*;

pub struct World<'a> {
    rigid_bodies: RigidBodySet,
    colliders: ColliderSet,
    integration_parameters: IntegrationParameters,
    physics_pipeline: PhysicsPipeline,
    island_manager: IslandManager,
    broad_phase: BroadPhase,
    narrow_phase: NarrowPhase,
    joint_set: JointSet,
    ccd_solver: CCDSolver,
    gravity: Vector<f32>,
    hooks: &'a dyn PhysicsHooks<RigidBodySet, ColliderSet>,
    events: &'a dyn EventHandler
}

impl Default for World<'_>{
    fn default() -> Self {
        World::new()
    }
}

impl World<'_> {
    pub fn new() -> Self{
        Self {
            rigid_bodies: RigidBodySet::new(),
            colliders: ColliderSet::new(),
            integration_parameters: IntegrationParameters::default(),
            physics_pipeline: PhysicsPipeline::new(),
            island_manager: IslandManager::new(),
            broad_phase: BroadPhase::new(),
            narrow_phase: NarrowPhase::new(),
            joint_set: JointSet::new(),
            ccd_solver: CCDSolver::new(),
            gravity: vector![0.0, -9.81, 0.0],
            hooks: &(),
            events: &(),
        }
    }

    pub fn step(&mut self){
        self.physics_pipeline.step(
            &self.gravity,
            &self.integration_parameters,
            &mut self.island_manager,
            &mut self.broad_phase,
            &mut self.narrow_phase,
            &mut self.rigid_bodies,
            &mut self.colliders,
            &mut self.joint_set,
            &mut self.ccd_solver,
            self.hooks,
            self.events,
        );
    }
}
