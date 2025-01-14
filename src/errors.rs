use thiserror::Error;

#[derive(Debug, Error)]
pub enum FloodFileError {
    #[error("Invalid channel type provided.")]
    InvalidChannelType,

    #[error("An error has occurred acquiring channel.")]
    ChannelError(#[from] std::io::Error),

    #[error("The provided file is too large to reliably send.")]
    FileTooLarge,

    #[error("The provided packet is too large to send.")]
    PacketTooLarge,

    #[error("Unable to send ARP packet over the wire.")]
    FailedToSendArp,

    #[error("Unable to serialize ARP packet.")]
    FailedToSerializeArp,

    #[error("Unable to deserialize ARP packet.")]
    FailedToDeserializeArp,

    #[error("Unable to generate file-hash.")]
    UnableToGenerateHash,
}
