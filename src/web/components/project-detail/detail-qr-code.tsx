import { QRCodeSVG } from "qrcode.react";

interface DetailQrCodeProps {
  readonly url: string;
}

const DetailQrCode = ({ url }: DetailQrCodeProps) => (
  <div className="flex h-size-detail-card w-size-detail-card shrink-0 items-center justify-center rounded-2xl border border-border-default bg-background p-3">
    <QRCodeSVG value={url} size={76} bgColor="transparent" fgColor="white" />
  </div>
);

export { DetailQrCode };
