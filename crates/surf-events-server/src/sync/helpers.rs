pub fn instruction_matches_program(
    tx: &surf_client::ParsedTransaction,
    instruction: &surf_client::InstructionInfo,
    program_id: &solana_pubkey::Pubkey,
) -> bool {
    tx.message
        .account_keys
        .get(instruction.program_id_index as usize)
        .copied()
        == Some(*program_id)
}
