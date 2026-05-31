import { CTA } from "./components/cta";
import { Features } from "./components/features";
import { Hero } from "./components/hero";
import { Models } from "./components/models";
import { Regulated } from "./components/regulated";
import { Tools } from "./components/tools";

export default function HomePage() {
  return (
    <>
      <Hero />
      <Features />
      <Regulated />
      <Models />
      <Tools />
      <CTA />
    </>
  );
}
