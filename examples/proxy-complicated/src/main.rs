//! # Proxy-Complex — statically erasing `Run` / `Step`
//!
//! [`OrderComplex::place`] is generic only over `P: Proxy<…>`: it has no
//! dependency on `Run`, `Step`, a repository, or a transaction context.
//! Each usecase selects an execution model by constructing a concrete `P`:
//!
//! ```text
//! customer repo  ─┐
//! inventory repo ─┼── run_proxy! ────────────┐
//! order repo     ─┘                          │
//!                                               ├── OrderComplex::place<P>
//! customer repo  ─┐                            │
//! inventory repo ─┼── step_proxy! + one &mut C ┘
//! order repo     ─┘
//! ```
//!
//! `P` is monomorphized at compile time, so this abstraction introduces no
//! dynamic dispatch.

use poprako_orchestra::{Oper, Proxy, Run, Step};

pub struct Context {
    pub events: Vec<String>,
}

pub struct EnsureCustomer<'a> {
    pub customer_id: &'a str,
}

impl Oper for EnsureCustomer<'_> {
    type Output = ();
}

pub struct ReserveStock<'a> {
    pub sku: &'a str,
    pub quantity: u32,
}

impl Oper for ReserveStock<'_> {
    type Output = ();
}

pub struct CreateOrder<'a> {
    pub customer_id: &'a str,
    pub sku: &'a str,
    pub quantity: u32,
}

impl Oper for CreateOrder<'_> {
    type Output = u64;
}

/// A logical atomic operation. Its dependency is `P`, not a repository:
/// `Run` and `Step` have already been erased into `Proxy` by the usecase.
pub struct OrderComplex;

impl OrderComplex {
    pub async fn place<P>(
        proxy: &mut P,
        customer_id: &str,
        sku: &str,
        quantity: u32,
    ) -> Result<u64, String>
    where
        P: for<'a> Proxy<EnsureCustomer<'a>, Error = String>
            + for<'a> Proxy<ReserveStock<'a>, Error = String>
            + for<'a> Proxy<CreateOrder<'a>, Error = String>,
    {
        proxy.exec(&EnsureCustomer { customer_id }).await?;

        proxy.exec(&ReserveStock { sku, quantity }).await?;

        proxy
            .exec(&CreateOrder {
                customer_id,
                sku,
                quantity,
            })
            .await
    }
}

pub struct CustomerRepo;

impl Run<EnsureCustomer<'_>> for CustomerRepo {
    type Error = String;

    async fn run(&self, oper: &EnsureCustomer<'_>) -> Result<(), Self::Error> {
        if oper.customer_id.is_empty() {
            return Err("customer ID must not be empty".to_owned());
        }

        Ok(())
    }
}

impl Step<EnsureCustomer<'_>, Context> for CustomerRepo {
    type Error = String;

    async fn step(
        &self,
        context: &mut Context,
        oper: &EnsureCustomer<'_>,
    ) -> Result<(), Self::Error> {
        context
            .events
            .push(format!("ensure customer {}", oper.customer_id));

        Ok(())
    }
}

pub struct InventoryRepo;

impl Run<ReserveStock<'_>> for InventoryRepo {
    type Error = String;

    async fn run(&self, oper: &ReserveStock<'_>) -> Result<(), Self::Error> {
        if oper.quantity == 0 {
            return Err("quantity must be positive".to_owned());
        }

        Ok(())
    }
}

impl Step<ReserveStock<'_>, Context> for InventoryRepo {
    type Error = String;

    async fn step(
        &self,
        context: &mut Context,
        oper: &ReserveStock<'_>,
    ) -> Result<(), Self::Error> {
        context
            .events
            .push(format!("reserve {} x{}", oper.sku, oper.quantity));
        Ok(())
    }
}

pub struct OrderRepo;

impl Run<CreateOrder<'_>> for OrderRepo {
    type Error = String;

    async fn run(&self, oper: &CreateOrder<'_>) -> Result<u64, Self::Error> {
        Ok((oper.customer_id.len() + oper.sku.len() + oper.quantity as usize) as u64)
    }
}

impl Step<CreateOrder<'_>, Context> for OrderRepo {
    type Error = String;

    async fn step(
        &self,
        context: &mut Context,
        oper: &CreateOrder<'_>,
    ) -> Result<u64, Self::Error> {
        context.events.push(format!(
            "create order for {}: {} x{}",
            oper.customer_id, oper.sku, oper.quantity,
        ));
        Ok(1)
    }
}

/// The only `Run`-specific code is local proxy construction across three repos.
async fn place_via_run<CR, IR, OR>(
    customer_repo: &CR,
    inventory_repo: &IR,
    order_repo: &OR,
    customer_id: &str,
    sku: &str,
    quantity: u32,
) -> Result<u64, String>
where
    CR: for<'a> Run<EnsureCustomer<'a>, Error = String>,
    IR: for<'a> Run<ReserveStock<'a>, Error = String>,
    OR: for<'a> Run<CreateOrder<'a>, Error = String>,
{
    let mut proxy = poprako_orchestra::run_proxy! {
        customer_repo => for<'a> EnsureCustomer<'a>;
        inventory_repo => for<'a> ReserveStock<'a>;
        order_repo => for<'a> CreateOrder<'a>;
    };

    OrderComplex::place(&mut proxy, customer_id, sku, quantity).await
}

/// `StepProxy` owns `&mut Context` once, then each `exec` reborrows it only
/// inside the selected repository call. The complex function sees only `&mut P`.
async fn place_via_step<CR, IR, OR>(
    context: &mut Context,
    customer_repo: &CR,
    inventory_repo: &IR,
    order_repo: &OR,
    customer_id: &str,
    sku: &str,
    quantity: u32,
) -> Result<u64, String>
where
    CR: for<'a> Step<EnsureCustomer<'a>, Context, Error = String>,
    IR: for<'a> Step<ReserveStock<'a>, Context, Error = String>,
    OR: for<'a> Step<CreateOrder<'a>, Context, Error = String>,
{
    let mut proxy = poprako_orchestra::step_proxy! {
        context;
        customer_repo => for<'a> EnsureCustomer<'a>;
        inventory_repo => for<'a> ReserveStock<'a>;
        order_repo => for<'a> CreateOrder<'a>;
    };

    OrderComplex::place(&mut proxy, customer_id, sku, quantity).await
}

fn main() {
    let _ = place_via_run::<CustomerRepo, InventoryRepo, OrderRepo>;
    let _ = place_via_step::<CustomerRepo, InventoryRepo, OrderRepo>;
}
