import brandInit from "@/assets/brand-init.png";

const BrandPanel = () => {
  return (
    <div className="hidden w-2/3 items-center justify-center p-6 md:flex">
      <img
        src={brandInit}
        alt="Desert landscape at sunset"
        className="h-full w-full rounded-3xl object-cover"
      />
    </div>
  );
};

export { BrandPanel };
