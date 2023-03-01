use super::Block;
use jwst::Workspace as JwstWorkspace;
use yrs::UpdateSubscription;

pub struct Workspace {
    pub(crate) workspace: JwstWorkspace,
    pub(crate) _sub: Option<UpdateSubscription>,
}

impl Workspace {
    pub fn new(id: String) -> Self {
        Self {
            workspace: JwstWorkspace::new(id),
            _sub: None
        }
    }

    pub fn id(&self) -> String {
        self.workspace.id()
    }

    pub fn client_id(&self) -> u64 {
        self.workspace.client_id()
    }

    pub fn get(&self, block_id: String) -> Option<Block> {
        let workspace = self.workspace.clone();
        self.workspace.with_trx(|trx| {
            let block = self
                .workspace
                .get(&trx.trx, &block_id)
                .map(|b| Block::new(workspace, b));
            drop(trx);
            block
        })
    }

    pub fn create(&self, block_id: String, flavor: String) -> Block {
        let workspace = self.workspace.clone();
        self.workspace.with_trx(|mut trx| {
            let block = Block::new(workspace, trx.create(block_id, flavor));
            drop(trx);
            block
        })
    }
}