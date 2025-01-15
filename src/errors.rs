use thiserror::Error;

#[derive(Debug, Error)]
pub enum FloodFileError {
    #[error("invalid channel type provided.")]
    InvalidChannelType,

    #[error("an error has occurred acquiring channel.")]
    ChannelError(#[from] std::io::Error),

    #[error("the provided file is too large to reliably send.")]
    FileTooLarge,

    #[error("the provided packet is too large to send.")]
    PacketTooLarge,

    #[error("unable to send ARP packet over the wire.")]
    FailedToSendArp,

    #[error("unable to serialize ARP packet.")]
    FailedToSerializeArp,

    #[error("unable to deserialize ARP packet.")]
    FailedToDeserializeArp,

    #[error("unable to generate file-hash.")]
    UnableToGenerateHash,

    #[error("invalid path to save files.")]
    InvalidDestinationPath,
}
