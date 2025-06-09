#![allow(clippy::result_large_err)]

use anchor_lang::prelude::*;
use anchor_spl::{associated_token::AssociatedToken, token_interface::{self, Mint, TokenInterface,TokenAccount, TransferChecked}};

declare_id!("FE2HW9s2RmQrUkTVYjzY78Z4XDGHcjXTLB2jaqGyzZuR");

#[program]
pub mod vesting {
    use super::*;

    pub fn create_vesting_account(ctx:Context<CreateVestingAccount>, company_name: String) -> Result<()> {
        let vesting_account = &mut ctx.accounts.vesting_account;
        vesting_account.owner = ctx.accounts.user.key();
        vesting_account.mint = ctx.accounts.mint.key();
        vesting_account.treasury_token_account = ctx.accounts.treasury_account.key();
        vesting_account.company_name = company_name;
        vesting_account.treasury_bump = ctx.bumps.treasury_account;
        vesting_account.bump = ctx.bumps.vesting_account;
        Ok(())
    }

    pub fn create_employee_account(ctx: Context<CreateEmplyeeAccount>, _company_name: String, start_time: i64, end_time:i64, amount: u64, cliff_time: i64) -> Result<()> {
      let employee_account = &mut ctx.accounts.employee_account;
      employee_account.employee = ctx.accounts.employee.key();
      employee_account.start_time = start_time;
      employee_account.end_time = end_time;
      employee_account.vesting_account = ctx.accounts.vesting_account.key();
      employee_account.total_amount = amount;
      employee_account.withdrwal = 0;
      employee_account.cliff_time = cliff_time;
      employee_account.bump = ctx.bumps.employee_account;
        Ok(())
    }

    pub fn claim_amount(ctx: Context<ClaimAmount>, _company_name: String) -> Result<()> {
        let employee_account = &mut ctx.accounts.employee_account;

        let now = Clock::get()?.unix_timestamp;

        if now < employee_account.cliff_time {
            return Err(ErrorCode::ClaimNotAvailableYet.into())
        }

        let time_since_start = now.saturating_sub(employee_account.start_time);
        let total_vesting_time = employee_account.end_time.saturating_sub(employee_account.start_time);

        if total_vesting_time == 0 {
            return Err(ErrorCode::InvalidVestingPeriod.into())
        }

        let vested_amount = if now >= employee_account.end_time {
            employee_account.total_amount
        } else {
            match employee_account.total_amount.checked_mul(time_since_start as u64) {
                Some(amount) => amount / total_vesting_time as u64,
                None => {
                    return Err(ErrorCode::CalculationOverflow.into())
                }
            }
        };

        let claimable_amount = vested_amount.saturating_sub(employee_account.withdrwal);

        if claimable_amount == 0 {
            return Err(ErrorCode::NothingToClaim.into())
        }

        let transfer_cpi_accounts = TransferChecked {
            from : ctx.accounts.treasury_token_account.to_account_info(),
            mint : ctx.accounts.mint.to_account_info(),
            to : ctx.accounts.employee.to_account_info(),
            authority : ctx.accounts.treasury_token_account.to_account_info(),
        };
        let cpi_program = ctx.accounts.token_program.to_account_info();
        let signer_seeds: &[&[&[u8]]] = &[
            &[
                b"vesting_treasury",
                ctx.accounts.vesting_account.company_name.as_ref(),
                &[ctx.accounts.vesting_account.treasury_bump],
            ],
        ];
        let cpi_context = CpiContext::new(cpi_program, transfer_cpi_accounts).with_signer(
            signer_seeds
        );
        let decimals = ctx.accounts.mint.decimals;
        token_interface::transfer_checked(cpi_context, claimable_amount as u64, decimals)?;
        employee_account.withdrwal += claimable_amount;
    
    Ok(())
    }
   
}

#[derive(Accounts)]
#[instruction(company_name: String)]

pub struct ClaimAmount <'info> {
   #[account(
    mut,
    seeds = [company_name.as_bytes()],
    bump = vesting_account.bump,
    has_one = mint,
    has_one = treasury_token_account
   )]
   pub vesting_account: Account<'info, VestingAccount>,

   #[account(
    mut, 
    seeds = [b"employee_vesting", employee.key().as_ref(), vesting_account.key().as_ref()], 
    bump = employee_account.bump, 
    has_one = employee, 
    has_one = vesting_account)]
   pub employee_account: Account<'info, EmployeeAccount>,

   #[account(
    mut
    )]
  pub treasury_token_account: InterfaceAccount<'info,TokenAccount>,

  pub mint : InterfaceAccount<'info, Mint>,

  #[account(
    init_if_needed,
    payer = employee,
    associated_token::mint = mint,
    associated_token::authority = employee,
    associated_token::token_program = token_program
  )]
  pub employee_token_account: InterfaceAccount<'info, TokenAccount>,

   #[account(mut)]
   pub employee: Signer<'info>, 
    pub token_program: Interface<'info, TokenInterface>,
    pub associated_token_program: Program<'info, AssociatedToken>,

   pub system_program : Program<'info, System>
}

#[derive(Accounts)]
#[instruction( company_name: String)]
pub struct CreateEmplyeeAccount <'info> {
    #[account(
        has_one = owner,
    )]
    pub vesting_account: Account<'info, VestingAccount>,

    #[account(init, payer = owner, space = 8+ EmployeeAccount::INIT_SPACE, seeds = [b"employee_vesting", employee.key().as_ref(), vesting_account.key().as_ref()], bump)]
    pub employee_account : Account<'info, EmployeeAccount>,
 
    pub employee: SystemAccount<'info>,

    #[account(mut)]
    pub owner: Signer<'info>,

    pub system_program: Program<'info,System>

}

#[derive(Accounts)]
#[instruction(company_name: String)]
pub struct CreateVestingAccount<'info> {

    #[account(init, payer = user, space = 8 + VestingAccount::INIT_SPACE, seeds = [company_name.as_ref()], bump)]
    pub vesting_account: Account<'info, VestingAccount>,

    pub mint: InterfaceAccount<'info, Mint>,

    #[account(mut)]
    pub user: Signer<'info>,

    #[account(
        init,
        token::mint = mint,
        token::authority = treasury_account,
        seeds=[b"vesting_treasury", company_name.as_bytes()],
        bump,
        payer = user,
    )]
    pub treasury_account: InterfaceAccount<'info,TokenAccount>,

    pub system_program: Program<'info, System>,
    pub token_program: Interface<'info, TokenInterface>
}

#[account]
#[derive(InitSpace)]
pub struct EmployeeAccount {
    pub employee: Pubkey,
    pub start_time: i64,
    pub end_time: i64,
    pub cliff_time: i64,
    pub vesting_account: Pubkey,
    pub total_amount: u64,
    pub withdrwal: u64,
    bump: u8
}

#[account]
#[derive(InitSpace)]
pub struct VestingAccount {
    pub owner: Pubkey,
    pub mint: Pubkey,
    pub treasury_token_account: Pubkey,
    #[max_len(50)]
    pub company_name: String,
    pub treasury_bump: u8,
    pub bump: u8 
}

#[error_code]
pub enum ErrorCode {
    #[msg("Claiming is not available yet.")]
    ClaimNotAvailableYet,
    #[msg("There is nothing to claim.")]
    NothingToClaim,
    InvalidVestingPeriod,
    CalculationOverflow
}